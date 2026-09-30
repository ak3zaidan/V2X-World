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
    .replace("rate_veh_per_h: 30.0", "rate_veh_per_h: 1800.0")
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

  const after = await check("after the run");
  expect(after.cards).toBeGreaterThan(2);
  // One window a second; the first may be too thin to report.
  expect(after.windows).toBeGreaterThanOrEqual(SECONDS - 2);
  expect(after.windows).toBeLessThanOrEqual(SECONDS + 1);

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
