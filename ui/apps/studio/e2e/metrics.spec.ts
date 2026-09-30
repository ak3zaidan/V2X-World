/**
 * The metrics dashboard, driven in a real browser against the mock engine: open it (button and
 * the M key), find a metric, expand it, brush a range, read the statistics and a breakdown, export
 * the CSV and the PNG, follow a link to a breakdown, check both themes, close it.
 *
 * The run is left running, as the files sorted after this one expect.
 */

import { expect, test, type Page } from "@playwright/test";
import { readFileSync } from "node:fs";

async function streamingFor(page: Page, seconds: number): Promise<void> {
  await expect(page.getByTestId("connection-state")).toHaveAttribute("data-state", "streaming", { timeout: 60_000 });
  const playing = await page.getByTestId("play").count();
  if (playing > 0) await page.getByTestId("play").click();
  await expect
    .poll(
      async () =>
        page.evaluate(() => {
          const client = window.__vwpStudio?.engine.client;
          return client ? Number((client.poses as unknown as { simTimeNs: bigint }).simTimeNs) / 1e9 : 0;
        }),
      { timeout: 60_000 },
    )
    .toBeGreaterThan(seconds);
}

test("open, find, expand, brush, export and close the metrics dashboard", async ({ page }) => {
  const errors: string[] = [];
  page.on("console", (m) => {
    if (m.type() === "error") errors.push(m.text());
  });
  await page.goto("/");
  await streamingFor(page, 8);

  // --- open with the keyboard; the old strip is gone -------------------------------------------
  await page.locator("body").click({ position: { x: 5, y: 300 } });
  await page.keyboard.press("m");
  await expect(page.getByTestId("metrics-panel")).toBeVisible();
  expect(page.url()).toContain("#metrics");
  await expect(page.getByTestId("plots-strip")).toHaveCount(0);

  // Cards grouped by question; a card with data shows a value, one without says why.
  await expect(page.getByTestId("metrics-group-delivery")).toBeVisible();
  await expect(page.getByTestId("metric-card-pdr")).toHaveAttribute("data-has-data", "true", { timeout: 20_000 });
  await expect(page.getByTestId("metric-value-pdr")).not.toHaveText("—");
  const emptyBoxes = await page.locator('.mcard[data-has-data="false"]').evaluateAll((cards) =>
    cards.filter((c) => (c.querySelector(".mcard-empty")?.textContent ?? "").trim().length < 10).length,
  );
  expect(emptyBoxes, "every card without data says why").toBe(0);

  // --- find a metric by search ---------------------------------------------------------------
  await page.getByTestId("metrics-search").fill("channel busy");
  await expect(page.getByTestId("metric-card-cbr")).toBeVisible();
  await expect(page.getByTestId("metric-card-pdr")).toHaveCount(0);
  await page.getByTestId("metrics-search").fill("");

  // --- pin one: it leads the dashboard and reaches the header ---------------------------------
  await page.getByTestId("metric-pin-cbr").click();
  await expect(page.getByTestId("metrics-group-pinned").getByTestId("metric-card-cbr")).toBeVisible();

  // --- expand ---------------------------------------------------------------------------------
  await page.getByTestId("metric-open-pdr").first().click();
  await expect(page.getByTestId("metric-expanded")).toHaveAttribute("data-metric", "pdr");
  expect(page.url()).toContain("#metrics/pdr");
  await expect(page.getByTestId("metric-chart").locator("canvas")).toHaveCount(1, { timeout: 20_000 });
  await expect(page.getByTestId("metric-source")).toContainText("TR 36.885");
  const statsBefore = await page.getByTestId("metric-stats-pdr").locator("td").nth(1).innerText();
  expect(Number(statsBefore.replace(/,/g, ""))).toBeGreaterThan(3);

  // --- brush a range on the strip under the chart ----------------------------------------------
  const brush = page.getByTestId("metric-brush");
  const box = await brush.boundingBox();
  if (!box) throw new Error("no brush");
  await page.mouse.move(box.x + box.width * 0.25, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width * 0.5, box.y + box.height / 2, { steps: 8 });
  await page.mouse.move(box.x + box.width * 0.6, box.y + box.height / 2, { steps: 4 });
  await page.mouse.up();
  await expect.poll(() => page.url()).toMatch(/#metrics\/pdr\?from=[\d.]+&to=[\d.]+/);
  const rangeText = await page.getByTestId("metric-range-text").innerText();
  const [from, to] = rangeText.split(" s of ")[0].split("–").map(Number);
  expect(to - from).toBeGreaterThan(0);
  const statsAfter = Number((await page.getByTestId("metric-stats-pdr").locator("td").nth(1).innerText()).replace(/,/g, ""));
  expect(statsAfter, "the statistics cover the brushed range only").toBeLessThan(Number(statsBefore.replace(/,/g, "")));

  // A breakdown over that range: delivery against distance, pooled by the engine.
  await expect(page.getByTestId("breakdown-dist_bin")).toBeVisible();
  await expect(page.getByTestId("breakdown-distance-chart")).toBeVisible({ timeout: 20_000 });
  await expect(page.getByTestId("breakdown-node").locator("tbody tr").first()).toBeVisible();

  // --- export ---------------------------------------------------------------------------------
  const [csv] = await Promise.all([page.waitForEvent("download"), page.getByTestId("metric-export-csv").click()]);
  expect(csv.suggestedFilename()).toMatch(/^pdr_.*\.csv$/);
  const csvText = readFileSync(await csv.path(), "utf8");
  const lines = csvText.trim().split(/\r\n/);
  expect(lines[0]).toBe("t_s,pdr (ratio)");
  expect(lines.length).toBeGreaterThan(1);
  for (const line of lines.slice(1)) {
    const t = Number(line.split(",")[0]);
    expect(t).toBeGreaterThanOrEqual(from - 1e-6);
    expect(t).toBeLessThanOrEqual(to + 1e-6);
  }
  const [png] = await Promise.all([page.waitForEvent("download"), page.getByTestId("metric-export-png").click()]);
  expect(png.suggestedFilename()).toMatch(/^pdr_.*\.png$/);
  const bytes = readFileSync(await png.path());
  expect([...bytes.subarray(1, 4)].map((b) => String.fromCharCode(b)).join("")).toBe("PNG");

  // --- light theme: the same chart, readable --------------------------------------------------
  await page.evaluate(() => document.documentElement.setAttribute("data-theme", "light"));
  const inkLight = await page.getByTestId("metric-stats").evaluate((el) => getComputedStyle(el).color);
  await page.evaluate(() => document.documentElement.removeAttribute("data-theme"));
  const inkDark = await page.getByTestId("metric-stats").evaluate((el) => getComputedStyle(el).color);
  expect(inkLight).not.toBe(inkDark);

  // --- Escape collapses the chart, then closes the dashboard ----------------------------------
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("metric-expanded")).toHaveCount(0);
  await expect(page.getByTestId("metrics-panel")).toBeVisible();
  await page.getByTestId("metrics-close").click();
  await expect(page.getByTestId("metrics-panel")).toBeHidden();
  expect(page.url()).not.toContain("#metrics");

  // --- a link opens a breakdown directly ------------------------------------------------------
  await page.goto("/#metrics/pdr/dist_bin?from=1&to=6");
  await expect(page.getByTestId("metric-expanded")).toHaveAttribute("data-metric", "pdr", { timeout: 60_000 });
  await expect(page.getByTestId("breakdown-dist_bin")).toHaveClass(/highlighted/);
  await expect(page.getByTestId("metric-range-text")).toContainText("1–6 s");
  await page.getByTestId("metrics-close").click();
  await expect(page.getByTestId("metrics-panel")).toBeHidden();

  // Unpin, so the next file starts from a clean header.
  await page.getByTestId("metrics-button").click();
  await page.getByTestId("metric-pin-cbr").first().click();
  await page.keyboard.press("Escape");

  const ignorable = (t: string): boolean => (t.includes("WebGL") && t.includes("deprecat")) || t.includes("GPU stall");
  expect(errors.filter((t) => !ignorable(t))).toEqual([]);
});
