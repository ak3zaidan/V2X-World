/**
 * A vehicle followed after its run has finished: the HUD names the pseudonym the engine names.
 * `inspect.node {include: [certs]}` answers it on a finished run;
 * the 2026-11-01 session found the HUD's pseudonym line missing there while it showed during the
 * run, so a person inspecting a finished run could not see which certificate the vehicle signed
 * with.
 */

import { join } from "node:path";

import { expect, test, type Page } from "@playwright/test";

import { EngineProcess, REPO, open, runToEnd, status } from "./support.js";

const engine = new EngineProcess(join(REPO, "scenarios/tests/studio-hud30.yaml"));

test.beforeAll(async () => engine.start());
test.afterAll(async () => engine.stop());

async function certs(page: Page, node: number): Promise<{ pseudonym?: string | null } | null> {
  return page.evaluate(async (n) => {
    const e = window.__vwpStudio?.engine as unknown as { requestHttp(m: string, p: unknown, o: unknown): Promise<{ certs?: { pseudonym?: string | null } }> };
    return (await e.requestHttp("inspect.node", { node: n, include: ["certs"] }, { quiet: true })).certs ?? null;
  }, node);
}

test("a vehicle followed on a finished run names its pseudonym in the HUD", async ({ page }) => {
  await open(page);
  await runToEnd(page, "restart", 5 * 60_000);
  expect((await status(page)).state).toBe("finished");
  // The middle radio of the run, as the numbers collector picks it: one still in the run.
  const actor = await page.evaluate(() => {
    const e = window.__vwpStudio?.engine as unknown as { nodeByActor: Map<number, number> };
    const actors = [...e.nodeByActor.keys()].sort((a, b) => a - b);
    return actors[Math.floor(actors.length / 2)] ?? -1;
  });
  expect(actor, "a radio in the run").toBeGreaterThanOrEqual(0);
  await page.evaluate((a) => window.__vwpStudio?.engine.selectActor(a, "chase"), actor);
  await expect(page.getByTestId("obu-hud")).toBeVisible({ timeout: 20_000 });
  const node = await page.evaluate((a) => (window.__vwpStudio?.engine as unknown as { nodeByActor: Map<number, number> }).nodeByActor.get(a) ?? null, actor);
  expect(node).not.toBeNull();
  const answer = await certs(page, node as number);
  expect(typeof answer?.pseudonym, "the engine names one").toBe("string");
  await page.waitForTimeout(3_000);
  const state = await page.evaluate(() => {
    const w = window as unknown as { __vwpStudio?: { engine: { streaming: boolean } } };
    return { streaming: w.__vwpStudio?.engine.streaming };
  });
  console.log(`finished-hud: node ${String(node)} answer ${String(answer?.pseudonym)} streaming ${String(state.streaming)}`);
  const line = page.getByTestId("hud-pseudonym");
  await expect(line, "the HUD's pseudonym line").toBeVisible({ timeout: 10_000 });
  await expect(line).toHaveAttribute("title", new RegExp(String(answer?.pseudonym)));
  // It is still there a few seconds later: nothing clears it once the page has settled.
  await page.waitForTimeout(3_000);
  await expect(line).toHaveAttribute("title", new RegExp(String(answer?.pseudonym)));
});
