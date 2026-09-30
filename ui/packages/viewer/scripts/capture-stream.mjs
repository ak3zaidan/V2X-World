#!/usr/bin/env node
/**
 * Capture a real engine's VWP stream to a file the glitch hunter replays.
 *
 *   node scripts/capture-stream.mjs --url http://127.0.0.1:8791 --out /tmp/cap --seconds 60
 *
 * Connects to `/vwp/v1` with the full profile and no compression, resumes the run (the engine is
 * expected to have been started `--paused --speed 0`, so it produces frames as fast as it can), and
 * writes every binary frame it receives, with its arrival time, to `<out>/stream.vwpcap`. The world
 * payload the `Hello` names is fetched from `/world/<hash>.vwb` into `<out>/world.vwb`.
 *
 * The file format is deliberately trivial so the replay needs no dependency:
 *
 *   "VWPCAP1\n"                         8-byte magic
 *   repeat: f64 arrivalMs | u32 length | u8[length] frame     (little-endian, no padding)
 *
 * Arrival times are recorded but the hunter does not trust them: an unpaced debug engine delivers
 * frames in bursts that say nothing about a real session. It re-times the frames on their sim time
 * with a stated jitter model instead (`test/glitch/replay.ts`).
 *
 * Stops after `--seconds` of *sim* time past the first keyframe, or when the run finishes.
 */

import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const args = new Map();
for (let i = 2; i < process.argv.length; i += 2) args.set(process.argv[i].replace(/^--/, ""), process.argv[i + 1]);
const base = (args.get("url") ?? "http://127.0.0.1:8791").replace(/\/$/, "");
const out = args.get("out") ?? "./capture";
const seconds = Number(args.get("seconds") ?? "60");
const skip = Number(args.get("skip") ?? "0");

mkdirSync(out, { recursive: true });
const wsUrl = `${base.replace(/^http/, "ws")}/vwp/v1?profile=full&compress=none&v=1`;
const chunks = [];
let bytes = 0;
const t0 = performance.now();
let firstSimNs = -1n;
let lastSimNs = 0n;
let frames = 0;
let worldHash = null;

function hex(u8) {
  return Array.from(u8, (b) => b.toString(16).padStart(2, "0")).join("");
}

async function rpc(method, params = {}) {
  const res = await fetch(`${base}/rpc`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ jsonrpc: "2.0", id: 1, method, params }),
  });
  return res.json();
}

const ws = new WebSocket(wsUrl, ["vwp.v1"]);
ws.binaryType = "arraybuffer";

const done = new Promise((resolve) => {
  ws.addEventListener("close", () => resolve());
  ws.addEventListener("error", (e) => {
    console.error("socket error", e?.message ?? e);
    resolve();
  });
});

ws.addEventListener("message", async (ev) => {
  if (typeof ev.data === "string") {
    try {
      const msg = JSON.parse(ev.data);
      if (msg.method === "run.state" && (msg.params?.state === "finished" || msg.params?.state === "stopped")) ws.close();
    } catch {
      /* not JSON-RPC */
    }
    return;
  }
  const buf = new Uint8Array(ev.data);
  const dv = new DataView(buf.buffer, buf.byteOffset, buf.byteLength);
  // Frame header §2.1: magic u32 @0, major u16 @4, msg_type u16 @6, flags u16 @8, ... seq u64 @?, body.
  const msgType = dv.getUint16(6, true);
  if (msgType === 1 && worldHash === null) {
    worldHash = hex(buf.subarray(24 + 56, 24 + 88));
  }
  if (msgType === 2 || msgType === 3) {
    const sim = dv.getBigUint64(24, true);
    if (firstSimNs < 0n && msgType === 2) firstSimNs = sim;
    lastSimNs = sim;
  }
  const rec = new Uint8Array(12 + buf.byteLength);
  const rv = new DataView(rec.buffer);
  rv.setFloat64(0, performance.now() - t0, true);
  rv.setUint32(8, buf.byteLength, true);
  rec.set(buf, 12);
  chunks.push(rec);
  bytes += rec.byteLength;
  frames++;
  if (firstSimNs >= 0n && Number(lastSimNs - firstSimNs) / 1e9 >= seconds + skip) ws.close();
});

ws.addEventListener("open", async () => {
  // The run was started paused; let it go.
  const r = await rpc("run.resume");
  if (r.error) console.error("run.resume:", JSON.stringify(r.error));
});

const timer = setInterval(() => {
  const simS = firstSimNs >= 0n ? Number(lastSimNs - firstSimNs) / 1e9 : 0;
  console.error(`captured ${frames} frames, ${(bytes / 1e6).toFixed(1)} MB, sim ${simS.toFixed(1)} s`);
}, 5000);

await done;
clearInterval(timer);

const magic = new TextEncoder().encode("VWPCAP1\n");
const all = new Uint8Array(magic.byteLength + bytes);
all.set(magic, 0);
let o = magic.byteLength;
for (const c of chunks) {
  all.set(c, o);
  o += c.byteLength;
}
writeFileSync(join(out, "stream.vwpcap"), all);
if (worldHash) {
  const res = await fetch(`${base}/world/${worldHash}.vwb`);
  const world = new Uint8Array(await res.arrayBuffer());
  writeFileSync(join(out, "world.vwb"), world);
  console.error(`world ${worldHash.slice(0, 12)}… ${(world.byteLength / 1e6).toFixed(1)} MB`);
}
console.error(`wrote ${frames} frames (${(all.byteLength / 1e6).toFixed(1)} MB) to ${out}`);
