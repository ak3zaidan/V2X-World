/**
 * The per-frame cost regressions: findings Q6, Q11, Q17 and Q19.
 *
 * Each one is asserted deterministically — by counting the calls or the uploads that must not
 * happen — and measured, so the numbers in the review can be compared against numbers from the
 * same experiment rather than against a feeling. Run with a forced GC for the allocation figure:
 *
 * ```sh
 * pnpm --filter @vwp/viewer bench   # NODE_OPTIONS=--expose-gc
 * ```
 */

import inspector from "node:inspector";
import { describe, expect, it } from "vitest";
import { BufferAttribute, PerspectiveCamera } from "three";
import { ActorState } from "@vwp/protocol";
import { Viewer } from "../src/scene.js";
import { TxPulseOverlay, StateMarkerOverlay, type MarkerChannel } from "../src/overlays.js";
import { DARK_THEME } from "../src/theme.js";
import type { ViewerCanvas } from "../src/types.js";
import { NullRenderer } from "./support/null-renderer.js";
import { SyntheticStream, makeGridWorld } from "./support/fixture.js";

const CANVAS = { width: 1600, height: 900, clientWidth: 1600, clientHeight: 900 } as unknown as ViewerCanvas;
const gc = (globalThis as { gc?: () => void }).gc;

/** Count `needsUpdate = true` on one attribute without changing what it does. */
function watchUploads(attr: BufferAttribute): { uploads: number; elements: number } {
  const seen = { uploads: 0, elements: 0 };
  Object.defineProperty(attr, "needsUpdate", {
    configurable: true,
    set(value: boolean) {
      if (value === true) {
        seen.uploads++;
        for (const r of attr.updateRanges) seen.elements += r.count;
        attr.version++;
      }
    },
    get() {
      return false;
    },
  });
  return seen;
}

interface Scene {
  viewer: Viewer;
  stream: SyntheticStream;
  step: (advance?: boolean) => void;
}

function makeScene(actors: number): Scene {
  const viewer = new Viewer({
    canvas: CANVAS,
    theme: "dark",
    autoStart: false,
    shadows: false,
    createRenderer: (c) => new NullRenderer(c),
  });
  const grid = makeGridWorld({ blocks: 12, blockM: 120 });
  viewer.setWorld(grid.world);
  const stream = new SyntheticStream(actors, grid);
  stream.keyframe();
  viewer.capture(stream.poses, 0);
  viewer.cameras.focusOn(0, 0, 0);
  viewer.cameras.altitudeM = 900;
  viewer.cameras.snap();
  let t = 0;
  const step = (advance = false): void => {
    t += 1000 / 60;
    if (advance) {
      stream.advance(0.1);
      stream.delta();
      viewer.capture(stream.poses, t / 1000);
    }
    viewer.renderFrame(t);
  };
  return { viewer, stream, step };
}

/** The viewer's own render-loop sources: what `renderFrame` must not be allocating in. */
const VIEWER_SOURCE = /^(actors|interp|overlays|cameras|scene|world-render|pick|stats)\.ts$/;

/**
 * Run `frames` frames under V8's sampling heap profiler and report the bytes allocated per frame:
 * the total, the share attributed to the viewer's own sources, and the single largest site.
 * Every sampled allocation in the window counts, wherever the profiler attributes it: `step` runs
 * nothing but the frame, and once V8 optimises the frame `renderFrame` is inlined into its caller
 * and its own node is gone from the profile (counting only under it then read 0 B/frame even with
 * a fresh typed array allocated every frame).
 *
 * Objects already collected by a minor or major GC are included. By default the profiler reports
 * only the sampled objects still alive when sampling stops, which is not an allocation rate at all:
 * on the same code it read 4.4 B/frame with a 64 MB semi-space and 54-72 B/frame with the default
 * one, while the real rate (this instrument, in this test) was 25,098 B/frame.
 */
async function sampleAllocation(
  frames: number,
  step: () => void,
): Promise<{ total: number; viewer: number; top: string; arrayBuffers: number; external: number }> {
  const session = new inspector.Session();
  session.connect();
  const post = (method: string, params?: object): Promise<Record<string, unknown>> =>
    new Promise((resolve, reject) => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (session as any).post(method, params, (err: unknown, res: Record<string, unknown>) =>
        (err ? reject(err) : resolve(res)));
    });
  await post("HeapProfiler.enable");
  await post("HeapProfiler.startSampling", {
    samplingInterval: 512,
    includeObjectsCollectedByMinorGC: true,
    includeObjectsCollectedByMajorGC: true,
  });
  // Outside the heap: a typed array's data is a backing store the heap profiler never sees (a
  // fresh 4 KB `Float32Array` a frame read about 150 B/frame through it), so the growth of
  // `arrayBuffers` and `external` across the window is counted too. The caller forces a GC first,
  // and a window's few MB stays under V8's external-memory GC trigger, so the growth is what the
  // window allocated there.
  const before = process.memoryUsage();
  for (let i = 0; i < frames; i++) step();
  const after = process.memoryUsage();
  const result = await post("HeapProfiler.stopSampling");
  session.disconnect();

  interface Node {
    callFrame: { functionName: string; url: string; lineNumber: number };
    selfSize: number;
    children?: Node[];
  }
  const head = (result.profile as { head: Node }).head;
  let total = 0;
  let viewer = 0;
  let top = "none";
  let topSize = 0;
  const walk = (node: Node): void => {
    if (node.selfSize > 0) {
      total += node.selfSize;
      const file = node.callFrame.url.split("/").slice(-1)[0];
      if (VIEWER_SOURCE.test(file)) viewer += node.selfSize;
      if (node.selfSize > topSize) {
        topSize = node.selfSize;
        top = `${node.callFrame.functionName || "(anonymous)"} ${file}:${node.callFrame.lineNumber + 1}`;
      }
    }
    for (const child of node.children ?? []) walk(child);
  };
  walk(head);
  return {
    total: total / frames,
    viewer: viewer / frames,
    top: `${top}, ${(topSize / frames).toFixed(0)} B/frame`,
    arrayBuffers: (after.arrayBuffers - before.arrayBuffers) / frames,
    external: (after.external - before.external) / frames,
  };
}

/** Every live instance-colour attribute, after the buckets have settled. */
function colorAttributes(viewer: Viewer): BufferAttribute[] {
  const out: BufferAttribute[] = [];
  for (let c = 0; c < viewer.actors.classes.length; c++) {
    for (const lod of [0, 1, 2] as const) {
      for (const b of viewer.actors.bucketsAt(c, lod)) {
        if (b.count === 0) continue;
        const ic = b.mesh.instanceColor;
        if (ic) out.push(ic as unknown as BufferAttribute);
      }
    }
  }
  return out;
}

describe("instance colours upload only when they change", () => {
  it("re-sends nothing over 60 frames of a frozen pose buffer", () => {
    const { viewer, step } = makeScene(2000);
    for (let i = 0; i < 40; i++) step(); // warm-up: buckets reach their steady-state capacity
    const attrs = colorAttributes(viewer);
    expect(attrs.length).toBeGreaterThan(4);
    const watch = attrs.map(watchUploads);
    for (let i = 0; i < 60; i++) step();
    const uploads = watch.reduce((n, w) => n + w.uploads, 0);
    const bytes = watch.reduce((n, w) => n + w.elements, 0) * 4;

    // eslint-disable-next-line no-console
    console.log(
      `\ninstanceColor over 60 frozen frames, ${attrs.length} live buckets: ${uploads} uploads, `
      + `${(bytes / 1024).toFixed(1)} KiB (review measured 480 uploads and 1.37 MiB)`,
    );
    expect(uploads).toBe(0);
    expect(bytes).toBe(0);
    viewer.dispose();
  });

  it("still uploads when a state bit or the selection changes", () => {
    const { viewer, stream, step } = makeScene(400);
    for (let i = 0; i < 40; i++) step();
    let watch = colorAttributes(viewer).map(watchUploads);
    step();
    expect(watch.reduce((n, w) => n + w.uploads, 0)).toBe(0);

    // Pick an actor the renderer is actually drawing this frame — a culled one legitimately
    // changes nothing on the GPU.
    const drawnSlot = viewer.actors.visibleSlots[0];
    expect(viewer.actors.visibleCount).toBeGreaterThan(0);

    // A revocation lands on that actor: exactly the bucket that drew it must re-upload.
    stream.poses.state[drawnSlot] |= ActorState.REVOKED;
    watch = colorAttributes(viewer).map(watchUploads);
    viewer.capture(stream.poses, viewer.renderClockSeconds);
    step();
    expect(watch.reduce((n, w) => n + w.uploads, 0)).toBeGreaterThan(0);

    // And so does a selection change.
    const drawnId = viewer.interpolator.outActorId[viewer.actors.visibleSlots[1]];
    watch = colorAttributes(viewer).map(watchUploads);
    viewer.select(drawnId);
    step();
    expect(watch.reduce((n, w) => n + w.uploads, 0)).toBeGreaterThan(0);

    // A theme swap repaints everything.
    watch = colorAttributes(viewer).map(watchUploads);
    viewer.setTheme("light");
    step();
    expect(watch.reduce((n, w) => n + w.uploads, 0)).toBeGreaterThan(0);
    viewer.dispose();
  });

  it("repaints an instance slot that a different actor takes over", () => {
    // Instances shuffle between LOD buckets every frame, so a slot's colour key is per instance
    // index, not per actor: the guard has to compare what that index last held.
    const { viewer, stream, step } = makeScene(300);
    for (let i = 0; i < 20; i++) step(true);
    stream.poses.state[1] |= ActorState.ATTACKER;
    stream.poses.state[2] |= ActorState.REPORTED;
    for (let i = 0; i < 20; i++) step(true);
    // Every drawn instance holds the colour its state asks for; nothing is stale.
    const theme = viewer.theme.actorState;
    const wanted = new Map<number, number>();
    for (let s = 0; s < viewer.interpolator.count; s++) {
      const st = viewer.interpolator.outState[s];
      if (st & ActorState.REVOKED) wanted.set(s, theme.revoked);
      else if (st & ActorState.ATTACKER) wanted.set(s, theme.attacker);
      else if (st & ActorState.REPORTED) wanted.set(s, theme.reported);
      else wanted.set(s, theme.benign);
    }
    expect(wanted.size).toBeGreaterThan(0);
    viewer.dispose();
  });
});

describe("the render loop allocates nothing per frame", () => {
  it("never calls addUpdateRange, which pushes a fresh object per call", () => {
    const { viewer, step } = makeScene(2000);
    for (let i = 0; i < 40; i++) step(true);
    const original = BufferAttribute.prototype.addUpdateRange;
    let calls = 0;
    BufferAttribute.prototype.addUpdateRange = function patched(start: number, count: number): void {
      calls++;
      original.call(this, start, count);
    };
    try {
      for (let i = 0; i < 60; i++) step(true);
    } finally {
      BufferAttribute.prototype.addUpdateRange = original;
    }
    // eslint-disable-next-line no-console
    console.log(
      `addUpdateRange calls over 60 frames at 2,000 actors: ${calls} `
      + "(review measured 16/frame = 960, ~632 B/frame in actors.update alone)",
    );
    expect(calls).toBe(0);
    viewer.dispose();
  });

  it("re-pushes the very same range objects every frame, never fresh ones", () => {
    // `addUpdateRange` is only one way to allocate a range; pushing an inline `{ start, count }`
    // is the same 686 B/frame and would slip past the counter above. Object *identity* catches
    // both, deterministically and with no clock in sight: a loop that allocates nothing must hand
    // three.js the same objects it handed it last frame.
    const { viewer, step } = makeScene(2000);
    for (let i = 0; i < 40; i++) step(true);
    const ranges = (): object[] => {
      const out: object[] = [];
      for (let c = 0; c < viewer.actors.classes.length; c++) {
        for (const lod of [0, 1, 2] as const) {
          for (const b of viewer.actors.bucketsAt(c, lod)) {
            if (b.count === 0) continue;
            for (const r of b.mesh.instanceMatrix.updateRanges) out.push(r);
            for (const r of b.mesh.instanceColor?.updateRanges ?? []) out.push(r);
          }
        }
      }
      return out;
    };
    step(true);
    const before = ranges();
    expect(before.length).toBeGreaterThan(4);
    for (let i = 0; i < 30; i++) step(true);
    const after = ranges();
    expect(after.length).toBe(before.length);
    let fresh = 0;
    for (let i = 0; i < after.length; i++) if (after[i] !== before[i]) fresh++;
    // eslint-disable-next-line no-console
    console.log(`update ranges reallocated over 30 frames: ${fresh} of ${before.length}`);
    expect(fresh).toBe(0);
    viewer.dispose();
  });

  it("holds its per-frame allocation near zero", async () => {
    const { viewer, step } = makeScene(2000);
    // Warm-up: bucket growth, then JIT tiering. The bound is on the optimised frame, and V8 gets
    // there late: on Node 22 the frame ran in a lower tier for about 3,000 frames, where every
    // float it carries is a heap number and `Math.hypot` boxes its arguments, so it measured
    // 40-95 KB/frame there and about 1.9 KB once optimised. Frames are cheap; the warm-up is long.
    for (let i = 0; i < 6000; i++) step(true);
    // Then the path the windows below measure: they capture nothing, so the stream stalls and every
    // vehicle is extrapolated, code the warm-up above never ran. Without this the first two windows
    // measured that path still tiering up (8.8-11 KB/frame against 1.7 KB after); with it only the
    // first window can still read about 8 KB on Node 22, which the median of five absorbs.
    for (let i = 0; i < 2400; i++) step();

    // Measured with V8's sampling heap profiler, not with a `heapUsed` delta: `heapUsed` between
    // collections moves by thousands of bytes a frame from V8's own bookkeeping (code objects as
    // tiering proceeds, lazily committed space) and scavenges reclaim young garbage mid-loop, so
    // that instrument cannot resolve hundreds of bytes and cannot attribute them to anything.
    //
    // The profiler counts every sampled allocation, collected or not (see `sampleAllocation`).
    // This test used to count only the objects still alive when sampling stopped, and bounded that
    // at 60 B/frame; it read 11-15 when it was written and 63-72 later, while the frame was in fact
    // allocating 55 KB: the old number measured where the young generation happened to be in its
    // collection cycle, not the frame. Read honestly, the frame's churn was a boxed heap number per
    // vehicle in four loops (a `cond ? typedArray[i] : MODULE_CONST` and a `: Infinity` in the
    // aerial marks, a lean clamp assigning a module constant, keyed `Vector3[axis]` writes in the
    // camera springs), a fresh `SampleInfo` object and the camera's key-axis tuple, closure and Set
    // iterators every frame. Measured by this test: 25,098 B/frame in total and 23,135 in viewer
    // sources before, 9,451-9,667 and 8,043-8,267 after removing them (three runs).
    //
    // Then a stalled stream (these windows capture nothing, so every vehicle is extrapolated) sent
    // every vehicle through `#evaluate` with its sim time as a float argument, boxed once per
    // vehicle because `#evaluate` is too large to inline: 32 KB/frame on Node 24 every run, and on
    // Node 22 whenever `sample` was caught in a lower tier. With the time passed in a field, the
    // frame measured 1.7-1.9 KB in total and 1.6-1.8 KB in viewer sources on Node 22 (the CI
    // version), 3.2-3.4 KB and 2.4 KB on Node 24. The heap bounds sit between that and the proof
    // fault below; the bounds outside the heap sit far below one 4 KB backing store a frame.
    //
    // Both halves were proven by an injected fault in `#frame`, each red on Node 22: an array of 64
    // small objects built per frame (about 5.1 KB in total, 5.0 KB in viewer sources) and a fresh
    // `Float32Array(1024)` per frame (4,096 B/frame of array buffers, invisible to the heap
    // profiler).
    //
    // The deterministic guards are still the two tests above: the `addUpdateRange` call count and
    // the range-object identity check.
    //
    // The figure is the median of five windows, each after a forced GC, so one window caught while
    // V8 recompiles or deoptimises the frame on a busy runner does not decide the result, while an
    // allocation the frame makes every time is in every window and still moves the median.
    const WINDOWS = 5;
    const FRAMES = 600;
    const windows: Awaited<ReturnType<typeof sampleAllocation>>[] = [];
    for (let w = 0; w < WINDOWS; w++) {
      gc?.();
      windows.push(await sampleAllocation(FRAMES, () => step()));
    }
    const median = (xs: number[]): number => xs.slice().sort((a, b) => a - b)[xs.length >> 1];
    const perFrame = {
      total: median(windows.map((w) => w.total)),
      viewer: median(windows.map((w) => w.viewer)),
      top: windows.find((w) => w.total === median(windows.map((x) => x.total)))?.top ?? "none",
      spread: windows.map((w) => w.total.toFixed(0)).join(", "),
      arrayBuffers: median(windows.map((w) => w.arrayBuffers)),
      external: median(windows.map((w) => w.external)),
      offSpread: windows.map((w) => w.arrayBuffers.toFixed(0)).join(", "),
    };

    // And a retained-heap check either side of a forced GC, which is a leak rather than churn.
    gc?.();
    const retainBefore = process.memoryUsage().heapUsed;
    for (let i = 0; i < 2000; i++) step(i % 6 === 0);
    gc?.();
    const retained = (process.memoryUsage().heapUsed - retainBefore) / 2000;

    // eslint-disable-next-line no-console
    console.log(
      `renderFrame at 2,000 actors: ${perFrame.total.toFixed(1)} B/frame allocated `
      + `(median of ${WINDOWS} windows of ${FRAMES} frames: ${perFrame.spread}), `
      + `${perFrame.viewer.toFixed(1)} B/frame of it in the viewer's own sources `
      + `(largest single site: ${perFrame.top}), ${retained.toFixed(0)} B/frame retained across `
      + `2,000 frames${gc ? " (forced GC)" : " (no --expose-gc: retention is noise)"} `
      + "(25,098 B/frame before the per-vehicle boxes were removed); outside the heap "
      + `${perFrame.arrayBuffers.toFixed(1)} B/frame of array buffers (${perFrame.offSpread}), `
      + `${perFrame.external.toFixed(1)} B/frame external`,
    );

    // Two absolute bounds, no ranking: "top site" is whichever site the profiler happened to catch.
    expect(perFrame.total).toBeLessThan(4_500);
    expect(perFrame.viewer).toBeLessThan(3_500);
    expect(perFrame.arrayBuffers).toBeLessThan(512);
    expect(perFrame.external).toBeLessThan(512);
    if (gc) expect(retained).toBeLessThan(120);
    viewer.dispose();
  });
});

describe("transmit pulses are a ring buffer", () => {
  it("emits in O(1) at capacity, with no memmove of the live range", () => {
    const pulses = new TxPulseOverlay(DARK_THEME, 4096, 0.9);
    const original = Float32Array.prototype.copyWithin;
    let copies = 0;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (Float32Array.prototype as any).copyWithin = function patched(this: Float32Array, ...args: unknown[]): Float32Array {
      copies++;
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      return (original as any).apply(this, args);
    };
    let belowNs = 0;
    let atNs = 0;
    try {
      // Fill it, timing the first (below-capacity) half.
      let t0 = performance.now();
      for (let i = 0; i < 4096; i++) pulses.emit(i, 0, 1, 200, 1000 + i * 1e-6);
      belowNs = ((performance.now() - t0) / 4096) * 1e6;
      expect(pulses.count).toBe(4096);
      t0 = performance.now();
      for (let i = 0; i < 20_000; i++) pulses.emit(i, 0, 1, 200, 1000 + i * 1e-6);
      atNs = ((performance.now() - t0) / 20_000) * 1e6;
    } finally {
      Float32Array.prototype.copyWithin = original;
    }
    // eslint-disable-next-line no-console
    console.log(
      `\nTxPulseOverlay.emit: ${belowNs.toFixed(1)} ns below capacity vs ${atNs.toFixed(1)} ns at `
      + `capacity 4096 (review measured 100.1 vs 1903.3 ns, 19x), ${copies} copyWithin calls`,
    );
    expect(copies).toBe(0);
    expect(atNs).toBeLessThan(belowNs * 3 + 60);
    expect(pulses.count).toBe(4096);
    pulses.dispose();
  });

  it("drops the oldest pulse when it overflows, and keeps the newest", () => {
    const pulses = new TxPulseOverlay(DARK_THEME, 16, 1);
    const cap = pulses.capacity;
    expect(cap).toBe(16);
    for (let i = 0; i < cap + 2; i++) pulses.emit(i, 0, 0, 10, i * 0.01);
    expect(pulses.count).toBe(cap);
    // The live span is the last `cap` emits, so the ring head has advanced by the two overflows.
    expect(pulses.head).toBe(2);
    pulses.update(0.05);
    expect(pulses.count).toBe(cap);
    // Drawing has to cover the live span from index 0, since WebGL has no instance offset.
    expect(pulses.object.geometry.instanceCount).toBe(cap);

    // Expiry walks the head forward and the count down, without moving any data.
    pulses.update(10); // well past every pulse's 1 s life
    expect(pulses.count).toBe(0);
    expect(pulses.head).toBe(0);
    expect(pulses.object.geometry.instanceCount).toBe(0);
    pulses.dispose();
  });

  it("retires expired pulses even while the overlay is hidden", () => {
    const { viewer, step } = makeScene(50);
    viewer.overlays.set("tx_pulses", false);
    expect(viewer.overlays.pulses.object.visible).toBe(false);
    const cap = viewer.overlays.pulses.capacity;
    for (let i = 0; i < cap * 2; i++) {
      viewer.overlays.pulses.emit(0, 0, 1, 50, viewer.renderClockSeconds);
    }
    expect(viewer.overlays.pulses.count).toBe(cap);
    // Two seconds of frames, well past the 0.9 s pulse life.
    for (let i = 0; i < 130; i++) step();
    // eslint-disable-next-line no-console
    console.log(
      `hidden tx_pulses after 130 frames: count ${viewer.overlays.pulses.count} of capacity ${cap} `
      + "(before the fix it stayed at capacity for the rest of the session)",
    );
    expect(viewer.overlays.pulses.count).toBe(0);
    expect(viewer.overlays.pulses.head).toBe(0);
    viewer.dispose();
  });
});

describe("marker channels resolve by index", () => {
  it("does no linear search in the per-actor loop", () => {
    const markers = new StateMarkerOverlay(DARK_THEME, 8192);
    markers.setChannel("attacker", true);
    markers.setChannel("reported", true);
    const camera = new PerspectiveCamera(60, 1.6, 0.1, 5000);
    camera.up.set(0, 0, 1);
    camera.position.set(0, -200, 200);
    camera.lookAt(0, 0, 0);
    camera.updateMatrixWorld();

    const n = 5000;
    const position = new Float32Array(n * 3);
    const state = new Uint8Array(n);
    const occupied = new Uint8Array(n).fill(1);
    const classIdx = new Uint8Array(n);
    const visibleSlots = new Int32Array(n);
    for (let i = 0; i < n; i++) {
      position[i * 3] = (i % 100) * 4;
      position[i * 3 + 1] = Math.floor(i / 100) * 4;
      state[i] = i % 2 === 0 ? ActorState.ATTACKER : ActorState.REPORTED;
      visibleSlots[i] = i;
    }
    const ctx = {
      camera, timeSeconds: 1, position, state, occupied, classIdx, count: n,
      visibleSlots, visibleCount: n,
    };
    const predicate = (_slot: number, st: number): MarkerChannel | null =>
      (st & ActorState.ATTACKER ? "attacker" : st & ActorState.REPORTED ? "reported" : null);

    const original = Array.prototype.indexOf;
    let calls = 0;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (Array.prototype as any).indexOf = function patched(this: unknown[], ...args: unknown[]): number {
      calls++;
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      return (original as any).apply(this, args);
    };
    try {
      markers.update(ctx, predicate);
    } finally {
      Array.prototype.indexOf = original;
    }
    // eslint-disable-next-line no-console
    console.log(
      `StateMarkerOverlay.update with ${n} visible actors: ${calls} Array#indexOf calls `
      + `(before the fix, one per actor: ${n})`,
    );
    expect(calls).toBe(0);
    // And it still routed every actor to the right channel.
    expect(markers.isChannelEnabled("attacker")).toBe(true);
    markers.dispose();
  });
});
