/**
 * Put the shared mock engine back the way the next spec expects to find it: running, at t = 0, at
 * 1x.
 *
 * One mock engine serves every spec of the run, so a spec that seeks, pauses or changes the speed
 * puts them back after each test. The reset goes through Playwright's own HTTP client (the Vite
 * server proxies `/rpc` to the engine), not through the page: when the page is what failed, hung
 * behind its renderer, a reset made inside it never ran, and every file after it found the engine
 * at 4x or paused.
 */

import { expect, type APIRequestContext } from "@playwright/test";

async function call(request: APIRequestContext, method: string, params: Record<string, unknown> = {}): Promise<{ result?: Record<string, unknown>; error?: { message?: string } }> {
  const res = await request.post("/rpc", { data: { jsonrpc: "2.0", id: 1, method, params }, timeout: 15_000 });
  expect(res.ok(), `${method}: HTTP ${res.status()}`).toBe(true);
  return (await res.json()) as { result?: Record<string, unknown>; error?: { message?: string } };
}

export async function resetEngine(request: APIRequestContext): Promise<void> {
  const status = await call(request, "run.status");
  if (status.result?.state === "finished") await call(request, "run.start");
  for (const [method, params] of [
    ["run.seek", { t_ns: 0, pause_after: false }],
    ["run.speed", { speed: 1 }],
  ] as const) {
    const r = await call(request, method, params);
    expect(r.error, `${method} after the test`).toBeUndefined();
  }
  // Refused when the run is not paused, which is the state wanted.
  await call(request, "run.resume");
  const after = await call(request, "run.status");
  expect(after.result?.state, "the engine is running again").toBe("running");
  expect(after.result?.speed, "the engine is at 1x again").toBe(1);
}
