/**
 * The inspector, the chase HUD, the message feed, the transport bar and the Backend panel in the
 * 2026-09-30 layout, against the real engine on a credential-system run.
 *
 * The scenario is `credential-lifecycle.yaml` (the US SCMS on a Midtown-sized grid, one roadside
 * unit placed by `position_m`), with two planned demand events at 12 s and 18 s so the timeline
 * has ticks on it. Each check names the defect it holds shut:
 *
 *  * the HUD said "(indices pending — node.tx)" for a whole SCMS run — it now names i and j;
 *  * the inspector printed "n/a" for every field the engine does not model — it now hides them;
 *  * "radios 82" beside "106 vehicles or roadside units" — the counts are now named for what they
 *    count, and the roadside unit placed by position is among the radios (the live Hello lists it);
 *  * live message rows moved under the cursor — they now hold still while the pointer is over them;
 *  * the transport bar carried thirteen controls — it is one row: play, step, speed, the clock and
 *    a timeline whose event ticks can be clicked;
 *  * the Backend panel covered the header — it is a shell panel under the header, like Metrics.
 */

import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { expect, test, type Page } from "@playwright/test";

import { EngineProcess, REPO, open, status } from "./support.js";

let engine: EngineProcess;

test.beforeAll(async () => {
  const source = readFileSync(join(REPO, "scenarios/credential-lifecycle.yaml"), "utf8");
  const dir = join(tmpdir(), `vwp-engine-e2e-inspector-${process.pid}`);
  mkdirSync(dir, { recursive: true });
  const path = join(dir, "e2e-inspector.yaml");
  // Two planned events, so the timeline has ticks to click whatever the run does.
  const text = `${source.replace("period_s: 300.0", "period_s: 10.0")}
events:
  - t: 12.0
    until: 16.0
    type: demand.multiplier
    value: 1.5
  - t: 18.0
    type: demand.multiplier
    value: 1.0
`;
  expect(text, "the scenario template changed; this test's edits no longer apply").not.toBe(source);
  writeFileSync(path, text);
  engine = new EngineProcess(path);
  await engine.start();
});

test.afterAll(async () => {
  await engine?.stop();
});

/** Play at real time until `seconds` of simulated time, then pause. */
async function playTo(page: Page, seconds: number): Promise<void> {
  await page.getByTestId("speed").selectOption("1");
  if ((await status(page)).state !== "running") await page.getByTestId("play").click();
  await expect.poll(async () => (await status(page)).t_ns, { timeout: 180_000 }).toBeGreaterThan(seconds * 1e9);
}

/** A vehicle carrying a radio, from the page's own node table. */
async function followARadio(page: Page): Promise<number> {
  let actor = -1;
  await expect
    .poll(
      async () => {
        actor = await page.evaluate(() => {
          const e = window.__vwpStudio?.engine as unknown as { nodeByActor: Map<number, number> };
          const actors = [...e.nodeByActor.keys()];
          return actors.length > 0 ? actors[0] : -1;
        });
        return actor;
      },
      { timeout: 60_000 },
    )
    .toBeGreaterThanOrEqual(0);
  await page.evaluate((a) => window.__vwpStudio?.engine.selectActor(a, "chase"), actor);
  return actor;
}

test("the inspector and the chase HUD show identity, radio, security and queues, and nothing empty", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("vwp.studio.devDetails", "1"));
  await open(page);

  // --- the empty state counts what it says it counts ---------------------------------------------
  await playTo(page, 6);
  await page.getByTestId("inspector-toggle").click();
  await page.getByTestId("tab-state").click();
  const radios = page.getByTestId("inspector-radios");
  await expect(radios).toContainText("roadside unit", { timeout: 30_000 });
  await expect(page.getByTestId("inspector-road-users")).toContainText("road user");
  // The unit placed by `position_m` is a roadside unit to the developer chip too.
  await expect(page.getByTestId("world-chip")).toContainText("1 RSU");

  // --- follow a vehicle ----------------------------------------------------------------------------
  await followARadio(page);
  await expect(page.getByTestId("obu-hud")).toBeVisible({ timeout: 30_000 });
  // The pseudonym is named with its certificate's indices, from the node's own security row.
  await expect(page.getByTestId("hud-pseudonym")).toContainText(/i \d+ · j \d+/, { timeout: 30_000 });
  await expect(page.getByTestId("hud-pseudonym")).not.toContainText("pending");
  await expect(page.getByTestId("hud-sec-pool")).toContainText("valid");
  await expect(page.getByTestId("hud-row-radio")).toBeVisible();

  await page.getByTestId("tab-state").click();
  await expect(page.getByTestId("insp-identity")).toBeVisible();
  await expect(page.getByTestId("inspector-cert-indices")).toHaveText(/^i \d+ · j \d+$/);
  await expect(page.getByTestId("insp-radio")).toBeVisible();
  await expect(page.getByTestId("insp-security")).toBeVisible();
  await expect(page.getByTestId("inspector-pool")).toContainText("valid");
  // A top-up is either on its way or has an instant.
  await expect(page.getByTestId("inspector-topup")).toHaveText(/in flight|in \d|expired/);

  // Nothing is drawn as "n/a", in the overview or in the HUD.
  const overview = (await page.getByTestId("inspector-state").innerText()) + (await page.getByTestId("obu-hud").innerText());
  expect(overview, "a field with no data was drawn as n/a").not.toMatch(/\bn\/a\b/);
  expect(overview).not.toContain("not on the wire");
  await page.screenshot({ path: join(tmpdir(), "vwp-e2e-inspector-overview.png") });
});

test("the message feed holds still under the pointer and catches up when it leaves", async ({ page }) => {
  await open(page);
  await playTo(page, 3);
  await followARadio(page);
  await page.getByTestId("tab-messages").click();
  await page.getByTestId("feed-tab-sent").click();
  const scroll = page.getByTestId("feed-scroll-sent");
  await expect(scroll).toBeVisible({ timeout: 60_000 });
  const firstMsg = async (): Promise<string | null> => page.getByTestId("feed-row-sent").first().getAttribute("data-msg");

  // Over the list: the top row stays the top row while the run keeps sending at 10 Hz.
  await scroll.hover();
  await expect(page.getByTestId("feed-status")).toContainText("held while the pointer is over the list");
  const held = await firstMsg();
  await page.waitForTimeout(2_500);
  expect(await firstMsg(), "a row moved under the pointer").toBe(held);
  await expect(page.getByTestId("feed-status")).toContainText("waiting");

  // Off the list: what arrived meanwhile is applied.
  await page.mouse.move(5, 5);
  await expect.poll(firstMsg, { timeout: 10_000 }).not.toBe(held);
  await expect(page.getByTestId("feed-status")).toContainText("live");

  // Queues: every cell says something or is blank; none says "n/o" or "n/a".
  await page.getByTestId("feed-tab-queues").click();
  await expect(page.getByTestId("feed-queues")).toBeVisible();
  const queues = await page.getByTestId("feed-queues").innerText();
  expect(queues).not.toMatch(/\bn\/[ao]\b/);
});

test("the transport bar is one row and its event ticks jump to the event", async ({ page }) => {
  await open(page);
  await playTo(page, 22);
  await page.getByTestId("pause").click();
  await expect.poll(async () => (await status(page)).state).toBe("paused");

  const bar = page.getByTestId("time-controls");
  expect(await bar.locator(":scope > button").count(), "play or pause, and step").toBe(2);
  expect(await bar.locator(":scope > select").count(), "the speed").toBe(1);
  for (const gone of ["seek-start", "step-back", "step-unit", "next-event", "event-list-button"]) {
    await expect(page.getByTestId(gone), `${gone} is still on the bar`).toHaveCount(0);
  }
  const play = await page.getByTestId("play").boundingBox();
  const scrub = await page.getByTestId("scrub").boundingBox();
  expect(Math.abs(scrub!.y + scrub!.height / 2 - (play!.y + play!.height / 2)), "the timeline is on the play button's row").toBeLessThan(8);

  // The scenario's two planned events are ticks on the timeline; a click on one moves the clock
  // to it. (They used to sit under the range input, where no pointer could reach them.)
  const marks = page.getByTestId("scenario-event-mark");
  await expect(marks).toHaveCount(2);
  await marks.first().click();
  await expect.poll(async () => (await status(page)).t_ns, { timeout: 60_000 }).toBe(12_000_000_000);
  await expect(page.getByTestId("sim-clock")).toHaveText(/^00:00:12/);

  // Alt+arrow on the timeline jumps between events from the keyboard.
  await page.getByTestId("scrub-range").focus();
  await page.keyboard.press("Alt+ArrowRight");
  await expect.poll(async () => (await status(page)).t_ns, { timeout: 60_000 }).toBe(18_000_000_000);
  // The jump disabled the bar while it ran; the keyboard must still be on the timeline afterwards,
  // or the next Alt+arrow goes nowhere (it did: the second jump never happened).
  await expect(page.getByTestId("scrub-range"), "a jump took the keyboard away from the timeline").toBeFocused();
  await page.keyboard.press("Alt+ArrowLeft");
  await expect.poll(async () => (await status(page)).t_ns, { timeout: 60_000 }).toBe(12_000_000_000);

  // Space on Play starts the run and Space again pauses it: the keyboard moves from Play to the
  // Pause that replaces it, rather than to the page.
  await page.getByTestId("play").focus();
  await page.keyboard.press("Space");
  await expect.poll(async () => (await status(page)).state).toBe("running");
  await expect(page.getByTestId("pause"), "starting the run took the keyboard off the bar").toBeFocused();
  await page.keyboard.press("Space");
  await expect.poll(async () => (await status(page)).state).toBe("paused");
});

test("the Backend panel opens under the header, reads at a glance and links to the metrics", async ({ page }) => {
  await open(page);
  await playTo(page, 8);
  await page.getByTestId("backend-button").click();
  const panel = page.getByTestId("panel-backend");
  await expect(panel).toBeVisible();
  // The header stays: the run's state, the clock and Pause are never behind the panel.
  await expect(page.getByTestId("header-scenario")).toBeVisible();
  const header = await page.locator("header.topbar").boundingBox();
  const box = await panel.boundingBox();
  expect(box!.y, "the Backend panel starts above the header's bottom edge").toBeGreaterThanOrEqual(header!.y + header!.height - 1);

  await expect(page.getByTestId("backend-glance")).toBeVisible({ timeout: 30_000 });
  await expect(page.getByTestId("backend-tile-messages")).toBeVisible();
  // Click the RA and follow its link to the metrics.
  await page.getByTestId("backend-entity-ra").click();
  await expect(page.getByTestId("backend-side")).toBeVisible();
  // The link appears once this page holds one of the RA's metrics, which a page that has just
  // joined a running engine receives at the next metric sample. It used to be optional here, and
  // when the sample had not arrived yet the test went on with the Backend panel still open, so the
  // button below closed it instead of opening it (2026-10-06): the link is now waited for.
  const link = page.getByTestId("backend-metrics-link");
  await expect(link, "the RA offers no link to its metrics").toBeVisible({ timeout: 60_000 });
  await link.click();
  await expect(page.getByTestId("panel-metrics")).toBeVisible();
  await expect(page.getByTestId("plots-strip")).toContainText("Measurements for");
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("panel-metrics")).toBeHidden();

  // The header's button opens the panel again, and Escape closes it, as it does the others.
  await page.getByTestId("backend-button").click();
  await expect(panel).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("panel-backend")).toHaveCount(0);
});
