/**
 * Live Studio evidence (docs/validation/live-studio-2026-10-25.md): settings changes made in the
 * Studio, as the owner makes them, on the real engine, and everything the page shows about each
 * run written down next to what the server's API says.
 *
 * The test collects; `tools/audit/studio_live.py` judges. It runs the base scenario and reads
 * every metric card, one metric's single-metric view and the PKI workspace's health strip with
 * the API answers they come from (`metrics.query`, `inspect.trust`, `inspect.pki`). Then, for
 * each variant, it types the variant's edits into the settings window, runs, reads the same,
 * opens "compare with the previous run" and reads every row beside `runs.compare`, and types the
 * base values back. The server records each run (`--record`); each run's recording is copied
 * aside when it finishes, so the checker can hold every number against the run's own
 * `metric.sample` records.
 *
 *   VWP_LIVE_SCENARIO  the base scenario (required)
 *   VWP_LIVE_VARIANTS  JSON {name: [[pointer, value, base value], ...]} (default {}: base only)
 *   VWP_LIVE_OUT       directory for evidence.json and the recordings (required)
 *   VWP_LIVE_EXPAND    the metric whose single-metric view is read (default pdr)
 */
import { copyFileSync, existsSync, mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

import { expect, test, type Page } from "@playwright/test";

import { EngineProcess, closeSettings, open, openSettings, runToEnd, setField, status, type Status } from "./support.js";

const scenario = process.env.VWP_LIVE_SCENARIO ?? "";
const outDir = process.env.VWP_LIVE_OUT ?? "";
const variants: Record<string, [string, string, string][]> = JSON.parse(process.env.VWP_LIVE_VARIANTS ?? "{}");
const expand = process.env.VWP_LIVE_EXPAND ?? "pdr";
const recording = outDir === "" ? "" : join(outDir, "recording.mcap");

const engine = new EngineProcess(scenario);

test.describe.configure({ mode: "serial" });
test.beforeAll(async () => {
  expect(scenario, "VWP_LIVE_SCENARIO").not.toBe("");
  expect(outDir, "VWP_LIVE_OUT").not.toBe("");
  mkdirSync(outDir, { recursive: true });
  process.env.VWP_ENGINE_ARGS = `${process.env.VWP_ENGINE_ARGS ?? ""} --record ${recording} --build-utc 2026-10-25T00:00:00Z`;
  await engine.start();
});
test.afterAll(async () => {
  await engine.stop();
});

/** A JSON-RPC call over the page's own HTTP client, as the panels make it. */
async function rpc(page: Page, method: string, params: unknown): Promise<unknown> {
  return page.evaluate(
    async ([m, p]) => {
      const e = window.__vwpStudio?.engine as unknown as { requestHttp(m: string, p: unknown, o: unknown): Promise<unknown> };
      try {
        return await e.requestHttp(m as string, p, { quiet: true });
      } catch (err) {
        return { __error: String(err) };
      }
    },
    [method, params] as const,
  );
}

/** `setField`, and a tick or untick for a boolean, which the settings window draws as a checkbox. */
async function edit(page: Page, pointer: string, value: string): Promise<void> {
  if (value === "true" || value === "false") {
    await openSettings(page);
    const filter = page.getByTestId("settings-filter");
    await filter.fill(pointer);
    const row = page.locator(`[data-testid="setting"][data-pointer="${pointer}"]`);
    await expect(row).toBeVisible();
    const box = row.locator('input[type="checkbox"]');
    if ((await box.count()) > 0) {
      await box.first().setChecked(value === "true");
      await filter.fill("");
      return;
    }
    await filter.fill("");
  }
  await setField(page, pointer, value);
}

async function closeSummary(page: Page): Promise<void> {
  const close = page.getByTestId("run-summary-close");
  await close.waitFor({ timeout: 10_000 }).catch(() => undefined);
  if (await close.isVisible()) await close.click();
}

/** Every card's shown value, read until two readings 1.5 s apart agree. */
async function readCards(page: Page): Promise<Record<string, string>> {
  const read = async (): Promise<Record<string, string>> =>
    page.evaluate(() => {
      const out: Record<string, string> = {};
      for (const el of document.querySelectorAll<HTMLElement>('[data-testid^="metric-value-"]')) {
        out[el.dataset.testid!.slice("metric-value-".length)] = el.textContent?.trim() ?? "";
      }
      for (const el of document.querySelectorAll<HTMLElement>('[data-testid^="metric-empty-"]')) {
        out[el.dataset.testid!.slice("metric-empty-".length)] = `EMPTY: ${el.textContent?.trim() ?? ""}`;
      }
      return out;
    });
  let last = await read();
  for (let i = 0; i < 20; i++) {
    await page.waitForTimeout(1500);
    const next = await read();
    if (JSON.stringify(next) === JSON.stringify(last) && Object.keys(next).length > 0) return next;
    last = next;
  }
  return last;
}

interface RunEvidence {
  status: Status;
  wall_ms: number;
  footprint_kb: number;
  threads: number;
  heap_bytes: number;
  period_ns: number;
  catalogue: unknown;
  api_series: unknown;
  cards: Record<string, string>;
  expanded: { metric: string; rows: Record<string, Record<string, string>> } | null;
  pki_strip: Record<string, string> | null;
  pki_trust: unknown;
  pki_vehicles: unknown;
  scenario: unknown;
  compare?: unknown;
}

async function collect(page: Page, wallMs: number): Promise<RunEvidence> {
  const st = await status(page);
  const periodNs = await page.evaluate(() => {
    const e = window.__vwpStudio?.engine as unknown as { client?: { hello?: { metricPeriodNs?: unknown } } };
    return Number(e.client?.hello?.metricPeriodNs ?? 0);
  });
  const catalogue = (await rpc(page, "metrics.query", {})) as { catalogue?: { name: string; base?: string; dims: string[] }[] };
  // The metrics panel: every card, then one metric's single-metric view over the whole run.
  await page.getByTestId("metrics-button").click();
  await expect(page.getByTestId("metrics-panel")).toBeVisible();
  const cards = await readCards(page);
  const headlines = (catalogue.catalogue ?? []).filter((c) => (c.base ?? c.name) === c.name).map((c) => c.name);
  const shown = headlines.filter((n) => n in cards);
  const apiSeries = await rpc(page, "metrics.query", { metrics: shown, t_from_ns: 0, t_to_ns: st.t_ns, bin_ns: periodNs > 0 ? periodNs : 1e9, limit: 100_000 });
  let expanded: RunEvidence["expanded"] = null;
  const opener = page.getByTestId(`metric-open-${expand}`);
  if ((await opener.count()) > 0) {
    await opener.click();
    await expect(page.getByTestId("metric-expanded")).toBeVisible();
    await page.getByTestId("metric-range-all").click().catch(() => undefined);
    await page.getByTestId("metric-stats").waitFor({ timeout: 20_000 }).catch(() => undefined);
    await page.waitForTimeout(2000);
    const rows = await page.evaluate(() => {
      const out: Record<string, Record<string, string>> = {};
      for (const tr of document.querySelectorAll<HTMLElement>('[data-testid^="metric-stats-"]')) {
        const id = tr.dataset.testid!.slice("metric-stats-".length);
        if (id === "unit" || id === "info") continue;
        const cells: Record<string, string> = {};
        for (const td of tr.querySelectorAll<HTMLElement>("td[data-stat]")) cells[td.dataset.stat!] = td.textContent?.trim() ?? "";
        out[id] = cells;
      }
      return out;
    });
    expanded = { metric: expand, rows };
    await page.getByTestId("metric-back").click();
  }
  await page.getByTestId("metrics-close").click();
  // The PKI workspace's health strip, and the answers it is computed from, at the same instant.
  let strip: Record<string, string> | null = null;
  const pkiButton = page.getByTestId("backend-button");
  if ((await pkiButton.count()) > 0 && (await pkiButton.isEnabled())) {
    await pkiButton.click();
    const health = page.getByTestId("pki-health");
    if (await health.waitFor({ timeout: 30_000 }).then(() => true, () => false)) {
      // The strip polls every 4 s; two polls after the run's end it shows the final state.
      await page.waitForTimeout(9000);
      strip = {};
      for (const k of ["certs", "pending", "cases", "crl", "revoked"]) {
        strip[k] = (await page.getByTestId(`pki-health-${k}`).locator("b").textContent())?.trim() ?? "";
      }
    }
    await page.getByTestId("pki-close").click().catch(() => undefined);
  }
  const heap = await page.evaluate(() => {
    (globalThis as { gc?: () => void }).gc?.();
    return (performance as unknown as { memory?: { usedJSHeapSize: number } }).memory?.usedJSHeapSize ?? 0;
  });
  return {
    status: st,
    wall_ms: wallMs,
    footprint_kb: engine.footprintKb(),
    threads: engine.threads(),
    heap_bytes: heap,
    period_ns: periodNs,
    catalogue: catalogue.catalogue ?? [],
    api_series: apiSeries,
    cards,
    expanded,
    pki_strip: strip,
    pki_trust: await rpc(page, "inspect.trust", {}),
    pki_vehicles: await rpc(page, "inspect.pki", { topic: "vehicles", view: "ground-truth" }),
    scenario: await rpc(page, "scenario.get", {}),
  };
}

/** Reads "compare with the previous run": every row, beside the server's own comparison. */
async function compareWithPrevious(page: Page): Promise<{ ui: unknown; api: unknown }> {
  const button = page.getByTestId("compare-previous-button");
  await expect(button).toBeVisible();
  await button.click();
  await expect(page.getByTestId("run-compare")).toBeVisible();
  const all = page.getByTestId("run-compare-all");
  if ((await all.count()) > 0) await all.check();
  await page.getByTestId("run-compare-metrics").waitFor({ timeout: 20_000 }).catch(() => undefined);
  await page.waitForTimeout(1500);
  const ui = await page.evaluate(() => {
    const rows: Record<string, { a: string; b: string; delta: string; pct: string; verdict: string }> = {};
    for (const tr of document.querySelectorAll<HTMLElement>('[data-testid^="run-compare-row-"]')) {
      const td = [...tr.querySelectorAll("td")].map((c) => c.textContent?.trim() ?? "");
      rows[tr.dataset.testid!.slice("run-compare-row-".length)] = { a: td[1], b: td[2], delta: td[3], pct: td[4], verdict: tr.dataset.verdict ?? "" };
    }
    const settings = document.querySelector('[data-testid="run-compare-settings"]')?.textContent ?? "";
    const noise = document.querySelector<HTMLElement>('[data-testid="run-compare-noise"]')?.dataset.sameSeed ?? "";
    return { rows, settings, same_seed: noise };
  });
  const api = await rpc(page, "runs.compare", { a: "previous", b: "current", format: "json" });
  await page.getByTestId("run-compare-close").click();
  return { ui, api };
}

test("settings changes in the Studio: every panel, the API and the records", async ({ page }) => {
  const runs: Record<string, RunEvidence> = {};
  // The runs in the order they ran (a variant whose edit could not be typed is left out, so
  // each comparison's "previous" is the run before it here), and what went wrong where.
  const order: string[] = [];
  const errors: Record<string, string> = {};
  const evidence = { scenario, variants, expand, runs, order, errors };
  const save = (): void => writeFileSync(join(outDir, "evidence.json"), JSON.stringify(evidence));
  await open(page);

  const one = async (name: string): Promise<void> => {
    const t0 = Date.now();
    await runToEnd(page, "run-start", 600_000);
    const wall = Date.now() - t0;
    order.push(name);
    await closeSummary(page);
    await closeSettings(page);
    runs[name] = await collect(page, wall);
    if (existsSync(recording)) copyFileSync(recording, join(outDir, `${name}.mcap`));
    save();
  };
  await one("A");
  for (const [name, list] of Object.entries(variants)) {
    try {
      for (const [pointer, value] of list) await edit(page, pointer, value);
      await one(name);
      runs[name].compare = await compareWithPrevious(page);
    } catch (e) {
      errors[name] = String(e).slice(0, 2000);
      await page.keyboard.press("Escape").catch(() => undefined);
    }
    save();
    for (const [pointer, , back] of list) await edit(page, pointer, back).catch((e) => (errors[`${name}:revert`] = String(e).slice(0, 500)));
  }
  save();
  expect(errors, "every variant typed, ran and was read").toEqual({});
});
