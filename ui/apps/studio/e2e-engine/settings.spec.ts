/**
 * The settings window, against the real engine: the owner asked for "a settings window that opens
 * up, like the VS Code settings", and this is each thing that sentence implies, pressed in the page.
 *
 *  1. Search finds a setting by name, by description and by path.
 *  2. Editing marks it edited; Undo edit restores the engine's value; Reset to default puts the
 *     engine's default in.
 *  3. The JSON view round-trips with the form, both ways.
 *  4. Apply reaches the next run: the run's horizon is the edited one.
 *  5. A value the engine refuses is marked at the field it names, with the engine's words.
 *  6. The Modified filter shows what the scenario sets and what was edited; Unsupported is a filter.
 *  7. The window lives in the address (#settings survives a reload), opens in a window of its own,
 *     and an Apply there reaches this page.
 */

import { expect, test } from "@playwright/test";

import { EngineProcess, closeSettings, open, openSettings, runToEnd, status, writeScenarios } from "./support.js";

const scenarios = writeScenarios();
const engine = new EngineProcess(scenarios.a);

test.describe.configure({ mode: "serial" });

test.beforeAll(async () => {
  await engine.start();
});
test.afterAll(async () => {
  await engine.stop();
});

const DURATION = '[data-testid="setting"][data-pointer="/time/duration_s"]';

test("search, edit, undo, reset, JSON round trip, apply to the next run", async ({ page }) => {
  await open(page);
  await openSettings(page);
  const search = page.getByTestId("settings-filter");
  const all = await page.getByTestId("setting").count();
  expect(all, "the window lists the engine's settings").toBeGreaterThan(50);

  // --- 1. search by name, description and path --------------------------------------------------
  await search.fill("duration");
  await expect(page.locator(DURATION)).toBeVisible();
  const byName = await page.getByTestId("setting").count();
  expect(byName, "a search narrows the list").toBeLessThan(all / 4);
  await search.fill("vehicles per hour");
  await expect(page.locator('[data-testid="setting"][data-pointer="/actors/vehicles/demand/rate_veh_per_h"]')).toBeVisible();
  await search.fill("time.duration_s");
  await expect(page.getByTestId("setting")).toHaveCount(1);
  await expect(page.getByTestId("settings-count")).toHaveText("1 found");
  await search.fill("zzzz no such setting");
  await expect(page.getByTestId("settings-empty")).toBeVisible();
  // The tree prunes to what the search shows.
  await search.fill("duration");
  const groups = await page.getByTestId("settings-tree-group").allTextContents();
  expect(groups.length, `tree while searching: ${groups.join(" | ")}`).toBeLessThan(5);

  // --- 2. edit, undo, reset -----------------------------------------------------------------------
  const row = page.locator(DURATION);
  const input = row.locator("input");
  const original = await input.inputValue();
  await input.fill("5");
  await expect(row).toHaveAttribute("data-edited", "true");
  await expect(page.getByTestId("settings-pending"), "the gear says an edit is waiting").toBeVisible();
  await expect(page.getByTestId("edit-route")).toContainText("1 unapplied edit");
  await row.getByTestId("setting-undo").click();
  await expect(input).toHaveValue(original);
  await expect(row).toHaveAttribute("data-edited", "false");
  // The scenario's 6 s is not the engine's default, so it is marked modified and can be reset.
  await expect(row).toHaveAttribute("data-modified", "true");
  const defaultText = (await row.locator(".setting-meta").innerText()).match(/Default: (\S+)/)?.[1] ?? "";
  expect(Number(defaultText), `the default line says ${defaultText}`).toBeGreaterThan(0);
  await row.getByTestId("setting-reset").click();
  await expect(input).toHaveValue(String(Number(defaultText)));
  await expect(row).toHaveAttribute("data-modified", "false");
  await expect(row).toHaveAttribute("data-edited", "true");
  await page.getByTestId("discard-edits").click();
  await expect(input).toHaveValue(original);

  // --- 3. the JSON view round-trips ---------------------------------------------------------------
  await input.fill("7");
  await page.getByTestId("settings-view-json").click();
  const json = page.getByTestId("settings-json");
  const text = await json.inputValue();
  const doc = JSON.parse(text) as { time: { duration_s: number } };
  expect(doc.time.duration_s, "a form edit is in the JSON").toBe(7);
  await json.fill(text.replace(/"duration_s": 7(\.0)?/, '"duration_s": 3'));
  await expect(page.getByTestId("settings-json-error")).toHaveCount(0);
  // Half-typed JSON is kept and does not reach the draft.
  await json.fill(`${text.slice(0, 40)}`);
  await expect(page.getByTestId("settings-json-error")).toBeVisible();
  await json.fill(text.replace(/"duration_s": 7(\.0)?/, '"duration_s": 3'));
  await page.getByTestId("settings-view-form").click();
  await search.fill("time.duration_s");
  await expect(page.locator(DURATION).locator("input"), "a JSON edit is in the form").toHaveValue("3");

  // --- 4. apply reaches the next run ------------------------------------------------------------
  await page.getByTestId("apply").click();
  await expect(page.getByTestId("scenario-message")).toContainText("Applied 1 change");
  await expect(page.getByTestId("staged-note")).toContainText("waiting for the next run");
  expect((await status(page)).staged_hash).not.toBeNull();
  const done = await runToEnd(page);
  expect(done.t_end_ns, "the run is the length the window set").toBe(3_000_000_000);
  await expect(page.getByTestId("settings-window"), "Run closes the window to show the run").toHaveCount(0);

  // --- 5. a refused value is marked at its field ------------------------------------------------
  await openSettings(page);
  await search.fill("time.duration_s");
  await page.locator(DURATION).locator("input").fill("-3");
  await page.getByTestId("apply").click();
  await expect(page.getByTestId("scenario-message")).toContainText("refused");
  const inline = page.locator(DURATION).getByTestId("setting-error");
  await expect(inline).toContainText("outside the allowed range");
  await expect(page.locator(DURATION)).toHaveAttribute("class", /invalid/);
  await expect(page.getByTestId("validation-err")).toContainText("/time/duration_s");
  await page.getByTestId("discard-edits").click();

  // --- 6. filters ----------------------------------------------------------------------------------
  await search.fill("");
  await page.getByTestId("settings-modified").click();
  const modified = await page.getByTestId("setting").evaluateAll((rows) =>
    rows.map((r) => [r.getAttribute("data-pointer"), r.getAttribute("data-modified"), r.getAttribute("data-edited")]),
  );
  expect(modified.length, "the scenario sets some values away from the default").toBeGreaterThan(0);
  expect(modified.every(([, m, e]) => m === "true" || e === "true"), JSON.stringify(modified)).toBe(true);
  expect(modified.map(([p]) => p)).toContain("/time/duration_s");
  await page.getByTestId("settings-modified").click();
  const unsupported = Number((await page.getByTestId("settings-unsupported").locator(".count").innerText()).trim());
  const shownBefore = await page.getByTestId("setting").count();
  await page.getByTestId("settings-unsupported").click();
  expect(await page.getByTestId("setting").count(), "Unsupported adds exactly the settings it counts").toBe(shownBefore + unsupported);
  await page.getByTestId("settings-unsupported").click();

  // --- 7a. the address keeps the window ---------------------------------------------------------
  expect(page.url()).toContain("#settings");
  await page.reload();
  await expect(page.getByTestId("settings-window"), "a reload on #settings reopens the window").toBeVisible({ timeout: 60_000 });
  await closeSettings(page);
  expect(page.url()).not.toContain("#settings");
});

test("the settings open in a window of their own, and an Apply there reaches this page", async ({ page, context }) => {
  await open(page);
  await openSettings(page);
  const [popup] = await Promise.all([context.waitForEvent("page"), page.getByTestId("settings-detach").click()]);
  await popup.waitForLoadState();
  await expect(page.getByTestId("settings-window"), "the main page's copy closes").toHaveCount(0);
  await expect(popup.getByTestId("settings-window")).toBeVisible({ timeout: 60_000 });
  await expect(popup.getByTestId("schema-source")).toContainText("each with what this engine does with it", { timeout: 60_000 });
  // It has no map and no stream: nothing in it is a canvas, and the engine sees no second viewer.
  await expect(popup.locator("canvas")).toHaveCount(0);

  await popup.getByTestId("settings-filter").fill("time.duration_s");
  await popup.locator(DURATION).locator("input").fill("2");
  await popup.getByTestId("apply").click();
  await expect(popup.getByTestId("scenario-message")).toContainText("Applied 1 change");
  // The main page hears of it without a reload: its gear shows applied settings waiting.
  await expect(page.getByTestId("settings-pending")).toBeVisible({ timeout: 15_000 });

  // Run in the detached window is started by the main page, which holds the stream.
  const before = (await status(page)).generation;
  await popup.getByTestId("run-start").click();
  await expect.poll(async () => (await status(page)).generation, { timeout: 60_000 }).toBeGreaterThan(before);
  await expect.poll(async () => (await status(page)).t_end_ns, { timeout: 60_000 }).toBe(2_000_000_000);
  await expect(page.getByTestId("connection-state")).toHaveAttribute("data-state", "streaming");
  await popup.close();
});
