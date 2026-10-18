/**
 * The PKI view's Vehicles, Cases and Revocation-lists tabs (WP-E3) in the real Studio, on the
 * real engine's answers for its recorded two-provider Manhattan run (`test/fixtures/pki/
 * pki-tabs.json`, which the mock engine serves for `inspect.pki` under `VWP_PKI_TABS`).
 *
 * The acceptance of 00-gap-analysis WP-E3, as a user sees it: the i × j grid draws the cells of
 * `node.credentials`, the case board's columns hold the cases at their `pki.case` stage, and the
 * view selector hides what only ground truth knows.
 */

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { expect, test, type Page } from "@playwright/test";

const tabs = JSON.parse(readFileSync(fileURLToPath(new URL("../test/fixtures/pki/pki-tabs.json", import.meta.url)), "utf8"));
const rows: { channel: string; record: Record<string, any> }[] = readFileSync( // eslint-disable-line @typescript-eslint/no-explicit-any
  fileURLToPath(new URL("../test/fixtures/pki/pki-run.jsonl", import.meta.url)), "utf8",
).trim().split("\n").map((l) => JSON.parse(l));

async function openPki(page: Page, tab: "vehicles" | "cases" | "crls"): Promise<void> {
  await page.goto("/");
  await expect(page.getByTestId("connection-state")).toHaveAttribute("data-state", "streaming", { timeout: 30_000 });
  await page.getByTestId("backend-button").click();
  await page.getByTestId(`pki-tab-${tab}`).click();
}

test("the Vehicles tab draws a vehicle's grid exactly as node.credentials lists it", async ({ page }) => {
  await openPki(page, "vehicles");
  const vehicles = tabs.vehicles.vehicles as { device: string; node: number }[];
  await expect(page.getByTestId("pki-vehicle-row")).toHaveCount(vehicles.length);
  const v = vehicles[0];
  await page.locator(`[data-testid="pki-vehicle-row"][data-node="${v.node}"]`).click();
  await expect(page.getByTestId("pki-grid")).toBeVisible();
  const last = rows.filter((r) => r.channel === "node.credentials" && r.record.device === v.device).at(-1)!.record;
  // Every cell the grid draws carries the state the record gives that (i, j).
  const drawn = await page.locator('[data-testid="pki-grid"] rect.cell').evaluateAll((els) =>
    els.map((e) => [Number(e.getAttribute("data-i")), Number(e.getAttribute("data-j")), e.getAttribute("data-state")]),
  );
  const want = new Map((last.cells as { i: number; j: number; state: string }[]).map((c) => [`${c.i}/${c.j}`, c.state]));
  expect(drawn.length).toBeGreaterThan(20 * 4);
  for (const [i, j, state] of drawn) expect(state, `${i}/${j}`).toBe(want.get(`${i}/${j}`));
  for (const c of last.cells as { i: number; j: number; state: string }[]) {
    if (c.state !== "not-yet-available") expect(drawn.some(([i, j]) => i === c.i && j === c.j), `${c.i}/${c.j} drawn`).toBe(true);
  }
  await expect(page.getByTestId("pki-pseudonyms")).toContainText(String(last.active));
});

test("the Cases board holds each case in its pki.case column, and an entity's view names no one", async ({ page }) => {
  await openPki(page, "cases");
  const furthest = new Map<string, string>();
  const order = ["reports-in", "corroborated", "decision", "plv-lookup", "la-seeds", "crl-published", "enforced"];
  for (const r of rows.filter((x) => x.channel === "pki.case")) {
    const prev = furthest.get(r.record.case);
    if (!prev || order.indexOf(r.record.stage) > order.indexOf(prev)) furthest.set(r.record.case, r.record.stage);
  }
  expect(furthest.size).toBeGreaterThan(0);
  for (const [id, stage] of furthest) {
    await expect(page.locator(`[data-testid="pki-board-col"][data-stage="${stage}"] [data-case="${id}"]`)).toBeVisible();
  }
  const first = [...furthest.keys()][0];
  await page.locator(`[data-case="${first}"]`).click();
  await expect(page.getByTestId("pki-case-truth")).toContainText("ee/");
  await expect(page.getByTestId("pki-case-learned")).toContainText("LA1");
  // Seen as LA1: its steps only, no reporter or accused, no ground-truth block.
  await page.getByTestId("pki-view").selectOption("provider-a/la1/0");
  await expect(page.getByTestId("pki-cases-as")).toBeVisible();
  await expect(page.getByTestId("pki-case-truth")).toHaveCount(0);
  await expect(page.getByTestId("pki-cases")).not.toContainText("ee/");
});

test("the Revocation-lists tab decodes a list and joins it to vehicles in ground truth only", async ({ page }) => {
  await openPki(page, "crls");
  const id8 = Object.keys(tabs.crl).find((k) => (tabs.crl[k]["ground-truth"].linkage as { matches: string[] }[]).some((e) => e.matches.length > 0))!;
  const head = (tabs.crls.crls as { id8: string; provider: string; series: number; kind: string }[]).find((c) => c.id8 === id8)!;
  await page.locator(`[data-testid="pki-crl-row"][data-key="${head.provider}/${head.series}/${head.kind}"]`).click();
  await page.locator(`[data-testid="pki-crl-version"][data-id8="${id8}"]`).click();
  await expect(page.getByTestId("pki-crl-entry")).toHaveCount(tabs.crl[id8]["ground-truth"].linkage.length);
  await expect(page.getByTestId("pki-crl-matches").first()).toContainText("ee/");
  await page.getByTestId("pki-view").selectOption("provider-a/ra/0");
  await expect(page.getByTestId("pki-crl-matches")).toHaveCount(0);
  await expect(page.getByTestId("pki-crl-card")).not.toContainText("ee/");
});
