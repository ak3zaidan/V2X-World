/**
 * The metrics dashboard against the real engine: live during a run, complete after it, and still
 * complete on a page reloaded after the run — which the last QA found showed no measurements at all,
 * because the page's only source was the stream it had watched.
 */

import { expect, test } from "@playwright/test";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { EngineProcess, REPO, open, runToEnd, status } from "./support.js";

const SECONDS = 12;

function scenario(): string {
  const text = readFileSync(join(REPO, "scenarios/phase1-grid.yaml"), "utf8")
    .replace("name: phase1-grid", "name: e2e-metrics")
    .replace("duration_s: 60.0", `duration_s: ${SECONDS}.0`)
    // Three vehicles a second: at 1,800 an hour this grid had three vehicles in twelve seconds, and
    // no one-second window reached pdr's 30-sample floor (it pooled 94 trials over the run).
    .replace("rate_veh_per_h: 30.0", "rate_veh_per_h: 10800.0")
    .replace("cols: 13", "cols: 4")
    .replace("rows: 34", "rows: 4");
  const dir = join(tmpdir(), `vwp-engine-metrics-${process.pid}`);
  mkdirSync(dir, { recursive: true });
  const path = join(dir, "e2e-metrics.yaml");
  writeFileSync(path, text);
  return path;
}

const engine = new EngineProcess(scenario());

test.beforeAll(async () => {
  await engine.start();
});
test.afterAll(async () => {
  await engine.stop();
});

test("the dashboard shows a finished run's measurements, and again after a reload", async ({ page }) => {
  await open(page);
  const done = await runToEnd(page);
  expect(done.t_end_ns).toBe(SECONDS * 1_000_000_000);

  const check = async (when: string): Promise<{ cards: number; windows: number }> => {
    await page.getByTestId("metrics-button").click();
    await expect(page.getByTestId("metrics-panel")).toBeVisible();
    await expect(page.getByTestId("metric-card-pdr"), `${when}: pdr has data`).toHaveAttribute("data-has-data", "true", { timeout: 30_000 });
    const cards = await page.locator('.mcard[data-has-data="true"]').count();
    await page.getByTestId("metric-open-pdr").first().click();
    await expect(page.getByTestId("metric-chart").locator("canvas")).toHaveCount(1, { timeout: 30_000 });
    const windows = Number((await page.getByTestId("metric-stats-pdr").locator("td").nth(1).innerText()).replace(/,/g, ""));
    await expect(page.getByTestId("breakdown-distance-chart")).toBeVisible({ timeout: 30_000 });
    await page.keyboard.press("Escape");
    await page.keyboard.press("Escape");
    await expect(page.getByTestId("metrics-panel")).toBeHidden();
    return { cards, windows };
  };

  // What the engine itself holds: every one-second window of pdr that carried a value. The
  // dashboard must show exactly these, no fewer (a page that only saw part of the stream) and no
  // more. The first seconds, before the vehicles are in, are below the 30-sample floor.
  const engineWindows = await page.evaluate(async () => {
    const engine = window.__vwpStudio?.engine as unknown as { requestHttp(m: string, p: unknown, o: unknown): Promise<{ rows: unknown[][] }> };
    const res = await engine.requestHttp("metrics.query", { metrics: ["pdr"], bin_ns: 1_000_000_000, limit: 10_000 }, { quiet: true });
    return res.rows.filter((r) => typeof r[1] === "number").length;
  });
  expect(engineWindows, "the engine measured pdr in most windows").toBeGreaterThanOrEqual(SECONDS / 2);

  const after = await check("after the run");
  expect(after.cards).toBeGreaterThan(2);
  expect(after.windows, "the dashboard shows every window the engine holds").toBe(engineWindows);

  await page.reload();
  await expect.poll(async () => (await status(page)).state, { timeout: 60_000 }).toBe("finished");
  const reloaded = await check("after a reload");
  expect(reloaded.cards, "a reloaded page shows the same measurements").toBe(after.cards);
  expect(reloaded.windows).toBe(after.windows);

  // A link straight to a breakdown, as the agent harness will write it.
  await page.goto("/#metrics/pdr/dist_bin?from=2&to=10");
  await expect(page.getByTestId("metric-expanded")).toHaveAttribute("data-metric", "pdr", { timeout: 60_000 });
  await expect(page.getByTestId("breakdown-dist_bin")).toHaveClass(/highlighted/);
  await expect(page.getByTestId("breakdown-distance-chart")).toBeVisible({ timeout: 30_000 });
  // eslint-disable-next-line no-console -- the measured numbers are the evidence this test reports
  console.log(`metrics dashboard: ${after.cards} cards with data, pdr over ${after.windows} windows; after reload ${reloaded.cards} and ${reloaded.windows}`);
});
