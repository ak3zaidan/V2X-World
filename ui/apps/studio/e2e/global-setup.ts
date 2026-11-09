/**
 * Opt-in CPU contention, to reproduce a slow CI runner on a fast machine.
 *
 * `VWP_E2E_CPU_HOGS=<n>` starts `n` busy-loop processes for the length of the run. WebGL here is
 * SwiftShader, a software rasteriser that runs on the CPU, so competing for the cores is what slows
 * it down: with one hog per core a 1,680 × 1,050 frame goes from about 200 ms to a second or more
 * on an 8-core laptop, and a five-draw pixel probe from about 1.5 s to 4-12 s — the regime the
 * hosted Ubuntu runners are in. DevTools CPU throttling does not reproduce it: it slows the page's
 * main thread, not the GPU process the rasteriser runs in.
 *
 * Off unless set; the processes are killed when the run ends.
 */

import { spawn, type ChildProcess } from "node:child_process";

export default function globalSetup(): () => void {
  const n = Math.max(0, Math.floor(Number(process.env.VWP_E2E_CPU_HOGS ?? "0")) || 0);
  const hogs: ChildProcess[] = [];
  for (let i = 0; i < n; i++) hogs.push(spawn(process.execPath, ["-e", "for (;;) {}"], { stdio: "ignore" }));
  return () => {
    for (const h of hogs) h.kill();
  };
}
