/**
 * Replays a captured engine stream (`scripts/capture-stream.mjs`) through a real {@link Viewer},
 * frame by frame, on a virtual clock — the harness the glitch hunter runs in.
 *
 * ## Timing model
 *
 * The capture's own arrival times come from an unpaced debug engine and are bursts, not a session,
 * so frames are re-timed on their **sim time**: frame `k` arrives at
 * `simTime_k / rate + latency + jitter`, with `latency` 8 ms, `jitter` uniform in `[0, jitterMs]`,
 * and, with probability `burstProb`, an extra `burstMs` (a GC pause or a TCP retransmission holding
 * a frame back). Arrivals stay in order, as a WebSocket delivers them. The render clock runs at
 * `fps` with ±0.5 ms of vsync noise, and with probability `dropProb` a frame takes two vsync
 * intervals (a dropped frame). Every draw is from a seeded generator, so a replay is repeatable.
 *
 * Frames are handed to the viewer exactly as `Viewer.attachClient` hands them — `applyHello`,
 * `applyKeyframe`/`applyDelta` and `capture` — between two rendered frames, which is when a browser
 * runs the socket's message events.
 */

import {
  MsgType,
  PoseBuffer,
  decodeDelta,
  decodeHello,
  decodeKeyframe,
  decodeWorld,
  viewFrame,
  type HelloMessage,
  type VwpWorld,
} from "@vwp/protocol";
import { Viewer } from "../../src/scene.js";
import type { FrameScheduler, ViewerCanvas } from "../../src/types.js";
import { GlitchHunter } from "../../src/glitch.js";
import { NullRenderer } from "../support/null-renderer.js";

/** One captured binary frame. */
export interface CaptureFrame {
  readonly msgType: number;
  /** Sim time of a keyframe or delta, seconds; NaN for anything else. */
  readonly simS: number;
  readonly bytes: ArrayBuffer;
}

/** Parse a `VWPCAP1` file. */
export function readCapture(file: Uint8Array): CaptureFrame[] {
  const magic = new TextDecoder().decode(file.subarray(0, 8));
  if (magic !== "VWPCAP1\n") throw new Error(`not a VWPCAP1 capture (magic ${JSON.stringify(magic)})`);
  const out: CaptureFrame[] = [];
  const dv = new DataView(file.buffer, file.byteOffset, file.byteLength);
  let o = 8;
  while (o + 12 <= file.byteLength) {
    const len = dv.getUint32(o + 8, true);
    const start = file.byteOffset + o + 12;
    const bytes = file.buffer.slice(start, start + len) as ArrayBuffer;
    o += 12 + len;
    const fdv = new DataView(bytes);
    const msgType = fdv.getUint16(6, true);
    const simS = msgType === MsgType.Keyframe || msgType === MsgType.Delta
      ? Number(fdv.getBigUint64(24, true)) / 1e9
      : Number.NaN;
    out.push({ msgType, simS, bytes });
  }
  return out;
}

/** mulberry32: a small seeded generator, so a replay is repeatable. */
export function rng(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

export interface ReplayOptions {
  /** Display refresh, Hz. Default 60. */
  readonly fps?: number;
  /** Playback rate, sim seconds per wall second. Default 1. */
  readonly rate?: number;
  readonly seed?: number;
  /** Arrival jitter, ms (uniform). Default 20. */
  readonly jitterMs?: number;
  /** Probability a frame is held back by `burstMs`. Default 0.02. */
  readonly burstProb?: number;
  readonly burstMs?: number;
  /** Probability a display frame takes two vsync intervals. Default 0.01. */
  readonly dropProb?: number;
  readonly width?: number;
  readonly height?: number;
  /** Passed to the viewer. */
  readonly logarithmicDepthBuffer?: boolean;
}

/** A virtual-clock scheduler: `now()` is whatever the replay says it is. */
class VirtualScheduler implements FrameScheduler {
  nowMs = 0;
  request(): number {
    return 0;
  }
  cancel(): void {}
  now(): number {
    return this.nowMs;
  }
}

/** The viewer, the hunter and the stream, advanced together on one virtual clock. */
export class StreamReplay {
  readonly viewer: Viewer;
  readonly hunter: GlitchHunter;
  readonly poses = new PoseBuffer(4096);
  readonly world: VwpWorld;
  hello: HelloMessage | null = null;
  #frames: CaptureFrame[];
  #arrival: Float64Array;
  #next = 0;
  #sched = new VirtualScheduler();
  #rand: () => number;
  #frameMs: number;
  #dropProb: number;
  /** Wall ms at which sim time zero of the first keyframe "arrived". */
  readonly firstSimS: number;
  readonly lastSimS: number;

  constructor(worldBytes: Uint8Array, frames: CaptureFrame[], options: ReplayOptions = {}) {
    const W = options.width ?? 1600;
    const H = options.height ?? 900;
    const canvas = { width: W, height: H, clientWidth: W, clientHeight: H } as unknown as ViewerCanvas;
    this.viewer = new Viewer({
      canvas,
      theme: "dark",
      autoStart: false,
      scheduler: this.#sched,
      logarithmicDepthBuffer: options.logarithmicDepthBuffer,
      createRenderer: (c) => new NullRenderer(c),
    });
    this.world = decodeWorld(worldBytes.buffer.slice(worldBytes.byteOffset, worldBytes.byteOffset + worldBytes.byteLength) as ArrayBuffer);
    this.viewer.setWorld(this.world);
    this.hunter = new GlitchHunter(this.viewer);
    this.#frames = frames;
    this.#rand = rng(options.seed ?? 0x5eed);
    const rate = options.rate ?? 1;
    const jitter = options.jitterMs ?? 20;
    const burstP = options.burstProb ?? 0.02;
    const burstMs = options.burstMs ?? 60;
    this.#frameMs = 1000 / (options.fps ?? 60);
    this.#dropProb = options.dropProb ?? 0.01;
    let first = Number.NaN;
    let last = Number.NaN;
    for (const f of frames) {
      if (Number.isFinite(f.simS)) {
        if (!Number.isFinite(first)) first = f.simS;
        last = f.simS;
      }
    }
    this.firstSimS = first;
    this.lastSimS = last;
    // Arrival times on the sim clock; frames without a sim time ride with the one before.
    this.#arrival = new Float64Array(frames.length);
    let prev = 0;
    let sim = first;
    for (let i = 0; i < frames.length; i++) {
      if (Number.isFinite(frames[i].simS)) sim = frames[i].simS;
      let a = ((sim - first) / rate) * 1000 + 8 + this.#rand() * jitter;
      if (this.#rand() < burstP) a += burstMs;
      if (a < prev) a = prev;
      this.#arrival[i] = a;
      prev = a;
    }
    this.#sched.nowMs = 0;
  }

  /** Virtual wall time, ms. */
  get nowMs(): number {
    return this.#sched.nowMs;
  }

  /** True once every frame has been delivered. */
  get exhausted(): boolean {
    return this.#next >= this.#frames.length;
  }

  #deliver(frame: CaptureFrame): void {
    const v = this.viewer;
    const view = viewFrame(frame.bytes);
    switch (frame.msgType) {
      case MsgType.Hello: {
        const hello = decodeHello(view);
        this.hello = hello;
        if (hello.actorCapacity > 0) {
          this.poses.ensureCapacity(Math.min(hello.actorCapacity, 4096));
          this.poses.setSlotBound(Math.min(hello.actorCapacity, 1 << 20));
        }
        v.applyHello(hello);
        v.capture(this.poses);
        break;
      }
      case MsgType.Keyframe: {
        const kf = decodeKeyframe(view);
        this.poses.applyKeyframe(kf);
        v.applyKeyframe(kf);
        v.capture(this.poses);
        this.hunter.observeSnapshot(this.poses);
        break;
      }
      case MsgType.Delta: {
        const d = decodeDelta(view);
        const r = this.poses.applyDelta(d);
        if (!r.applied) break;
        v.applyDelta(d);
        v.capture(this.poses);
        this.hunter.observeSnapshot(this.poses);
        break;
      }
      default:
        break;
    }
  }

  /** Deliver every frame that has arrived by now. */
  #pump(): void {
    while (this.#next < this.#frames.length && this.#arrival[this.#next] <= this.#sched.nowMs) {
      this.#deliver(this.#frames[this.#next]);
      this.#next++;
    }
  }

  /**
   * Advance one display frame: deliver what arrived, draw, let the hunter look. Returns false once
   * the stream is exhausted and the render clock has passed its end.
   */
  step(hunt = true): boolean {
    const drop = this.#rand() < this.#dropProb;
    const noise = (this.#rand() - 0.5) * 1.0;
    this.#sched.nowMs += this.#frameMs * (drop ? 2 : 1) + noise;
    this.#pump();
    this.viewer.renderFrame(this.#sched.nowMs);
    if (hunt) this.hunter.afterFrame();
    return !(this.exhausted && this.viewer.interpolator.renderSimSeconds >= this.lastSimS - 0.3);
  }

  /** Run `seconds` of display time. */
  run(seconds: number, hunt = true, each?: () => void): void {
    const end = this.#sched.nowMs + seconds * 1000;
    while (this.#sched.nowMs < end) {
      if (!this.step(hunt)) break;
      each?.();
    }
  }
}
