// Debug only (not committed): camera motion around each flight's landing.
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { it } from "vitest";
import { StreamReplay, readCapture } from "./glitch/replay.js";
import { runTour } from "./glitch/tour.js";

const dir = process.env.VWP_GLITCH_CAPTURE;
it.skipIf(!dir || !existsSync(join(dir, "stream.vwpcap")))("landing trace", () => {
  const frames = readCapture(readFileSync(join(dir!, "stream.vwpcap")));
  const replay = new StreamReplay(readFileSync(join(dir!, "world.vwb")), frames, { seed: 24593 });
  const v = replay.viewer;
  let wasTransit = false;
  let landedAt = -1;
  let frame = 0;
  const hist: string[] = [];
  let px = NaN, py = NaN, pz = NaN, pvx = NaN, pvy = NaN, pvz = NaN;
  const out: string[] = [];
  runTour(replay, {
    warmupS: Number(process.env.VWP_GLITCH_WARMUP ?? "30"), aerialS: 3, chaseS: 4, dashboardS: 4,
    onFrame: () => {
      frame++;
      const c = v.camera.position;
      const t = v.cameras.inTransit;
      const vx = c.x - px, vy = c.y - py, vz = c.z - pz;
      const jerk = Math.hypot(vx - pvx, vy - pvy, vz - pvz);
      const fs = v.followSlot;
      const sx = fs >= 0 ? v.interpolator.outPosition[fs * 3] : NaN;
      const sy = fs >= 0 ? v.interpolator.outPosition[fs * 3 + 1] : NaN;
      const line = `${frame} t=${(replay.nowMs / 1000).toFixed(3)} dt=${(v.lastFrame.dtSeconds * 1000).toFixed(1)} ${v.cameras.mode} transit=${t ? 1 : 0} cam=(${c.x.toFixed(2)},${c.y.toFixed(2)},${c.z.toFixed(2)}) v=(${(vx * 100).toFixed(1)},${(vy * 100).toFixed(1)},${(vz * 100).toFixed(1)})cm jerk=${(jerk * 100).toFixed(1)}cm subj=(${sx.toFixed(2)},${sy.toFixed(2)}) look=(${v.cameras.look.x.toFixed(2)},${v.cameras.look.y.toFixed(2)},${v.cameras.look.z.toFixed(2)}) fov=${v.camera.fov.toFixed(2)}`;
      hist.push(line);
      if (hist.length > 6) hist.shift();
      if (wasTransit && !t) { landedAt = frame; out.push("--- landing"); out.push(...hist); }
      else if (landedAt > 0 && frame - landedAt <= 30) out.push(line);
      wasTransit = t;
      pvx = vx; pvy = vy; pvz = vz; px = c.x; py = c.y; pz = c.z;
    },
  });
  process.stderr.write(out.join("\n") + "\n");
}, 600_000);
