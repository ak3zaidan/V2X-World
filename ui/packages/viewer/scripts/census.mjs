// node scripts/census.mjs <capture dir>: live actors per class in a capture (scripts/capture-stream.mjs),
// from its keyframes every 15 s of sim. Needs the protocol package built (pnpm --filter @vwp/protocol build).
import { readFileSync } from "node:fs";
import { join } from "node:path";
const P = new URL("../../protocol/dist/index.js", import.meta.url).href;
const { MsgType, decodeHello, decodeKeyframe, viewFrame } = await import(P);
const file = readFileSync(join(process.argv[2], "stream.vwpcap"));
const dv = new DataView(file.buffer, file.byteOffset, file.byteLength);
let o = 8;
let names = [];
let firstSim = -1;
const rows = [];
while (o + 12 <= file.byteLength) {
  const len = dv.getUint32(o + 8, true);
  const start = file.byteOffset + o + 12;
  const bytes = file.buffer.slice(start, start + len);
  o += 12 + len;
  const t = new DataView(bytes).getUint16(6, true);
  if (t === MsgType.Hello) {
    const h = decodeHello(viewFrame(bytes));
    names = Array.from(h.classes.strName, (s) => h.strings[s] ?? `#${s}`);
  } else if (t === MsgType.Keyframe) {
    const k = decodeKeyframe(viewFrame(bytes));
    const sim = Number(k.simTimeNs) / 1e9;
    if (firstSim < 0) firstSim = sim;
    const rel = sim - firstSim;
    if (Math.abs(rel - Math.round(rel / 15) * 15) > 0.05) continue;
    const by = {};
    let live = 0;
    for (let i = 0; i < k.actors.count; i++) {
      if (k.actors.actorId[i] === 0xffffffff) continue;
      live++;
      const n = names[k.actors.classIdx[i]] ?? "?";
      by[n] = (by[n] ?? 0) + 1;
    }
    rows.push(`t=${rel.toFixed(0)} s live ${live} ${JSON.stringify(by)}`);
  }
}
console.log(rows.join("\n"));
