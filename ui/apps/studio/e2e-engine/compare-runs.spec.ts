/**
 * "Compare with previous run" against a real engine: run, change one setting (the vehicles'
 * transmit power), run again, compare. The engine keeps the first run's digest when the second
 * starts (`runs.list`), and the comparison names the setting, says the seed is shared, and ranks
 * the metrics by their change. `VWP_SHOTS=<dir>` saves screenshots of the dialog there.
 */

import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { expect, test } from "@playwright/test";

import { EngineProcess, REPO, open, runToEnd, setField } from "./support.js";

function scenario(): string {
  // `VWP_COMPARE_SCENARIO`: a richer scenario (a city, every radio metric) for the screenshots.
  if (process.env.VWP_COMPARE_SCENARIO) return process.env.VWP_COMPARE_SCENARIO;
  const source = readFileSync(join(REPO, "scenarios/phase1-grid.yaml"), "utf8");
  const dir = join(tmpdir(), `vwp-compare-e2e-${process.pid}`);
  mkdirSync(dir, { recursive: true });
  const path = join(dir, "compare-grid.yaml");
  writeFileSync(path, source.replace("name: phase1-grid", "name: compare-grid").replace("duration_s: 60.0", "duration_s: 30.0"));
  return path;
}

const engine = new EngineProcess(scenario());
const shots = process.env.VWP_SHOTS;

test.describe.configure({ mode: "serial" });
test.beforeAll(async () => {
  await engine.start();
});
test.afterAll(async () => {
  await engine.stop();
});

test("run, change the transmit power, run again, compare with the previous run", async ({ page }) => {
  // The run summary opens itself when a run finishes; it sits over the header.
  const closeSummary = async (): Promise<void> => {
    const close = page.getByTestId("run-summary-close");
    await close.waitFor({ timeout: 10_000 }).catch(() => undefined);
    if (await close.isVisible()) await close.click();
  };
  await open(page);
  await runToEnd(page);
  await closeSummary();
  // One run: nothing to compare with yet.
  await expect(page.getByTestId("compare-previous-button")).toHaveCount(0);

  await setField(page, "/radio/devices/obu/tx_power_dbm", "10");
  await runToEnd(page);
  // The summary offers the comparison too.
  await expect(page.getByTestId("run-summary-compare")).toBeVisible();
  await closeSummary();

  const button = page.getByTestId("compare-previous-button");
  await expect(button).toBeVisible();
  await button.click();
  const dialog = page.getByTestId("run-compare");
  await expect(dialog).toBeVisible();
  await expect(page.getByTestId("run-compare-settings")).toContainText("10");
  await expect(page.getByTestId("run-compare-noise")).toHaveAttribute("data-same-seed", "true");
  await expect(page.getByTestId("run-compare-summary")).toContainText("setting");
  // The small grid may change nothing meaningfully; every metric is one tick away.
  if ((await page.getByTestId("run-compare-metrics").count()) === 0) await page.getByTestId("run-compare-all").check();
  await expect(page.getByTestId("run-compare-metrics")).toBeVisible();
  const pdr = page.getByTestId("run-compare-row-pdr");
  if ((await pdr.count()) > 0) {
    // Less power, shorter range: delivery cannot improve.
    await expect(pdr).not.toHaveAttribute("data-verdict", "better");
    await pdr.click();
  }
  await expect(page.getByTestId("run-compare-chart")).toBeVisible();
  if (shots) await dialog.screenshot({ path: join(shots, "compare-dialog.png") });

  await page.getByTestId("run-compare-all").check();
  await page.getByTestId("run-compare-metrics").scrollIntoViewIfNeeded();
  if (shots) await page.screenshot({ path: join(shots, "compare-all.png"), fullPage: false });
  await page.getByTestId("run-compare-close").click();
  await expect(dialog).toHaveCount(0);
});
