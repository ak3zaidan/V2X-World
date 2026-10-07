/**
 * The vehicle overlay against the real engine (owner points 1, 3, 5, 6 of 2026-10-06): select a
 * vehicle, read the slim HUD, open the overlay, see the BSMs it sent and received, read one (which
 * pauses the feed), and close it. No Log or Why tab exists.
 */

import { expect, test, type Page } from "@playwright/test";

import { status, streaming } from "./support.js";

const SHOTS = process.env.VWP_SHOTS ?? "test-results";

async function followARadio(page: Page): Promise<void> {
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
  await page.evaluate(async (a) => {
    const e = window.__vwpStudio?.engine as unknown as { selectActor(a: number, m: string): Promise<void> };
    await e.selectActor(a, "chase");
  }, actor);
}

test("select a vehicle, open its overlay, read its sent and received BSMs, close it", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto("/");
  await streaming(page);
  await page.getByTestId("speed").selectOption("1");
  if ((await status(page)).state !== "running") await page.getByTestId("play").click();
  await expect.poll(async () => (await status(page)).t_ns, { timeout: 180_000 }).toBeGreaterThan(4e9);

  await followARadio(page);
  const hud = page.getByTestId("obu-hud");
  await expect(hud).toBeVisible({ timeout: 30_000 });
  await expect(page.getByTestId("hud-row-radio")).toBeVisible({ timeout: 30_000 });
  await expect(page.getByTestId("hud-health")).toBeVisible();
  expect(await hud.innerText()).not.toMatch(/\bn\/a\b|CPU|HSM|DCC/);
  // Selecting does not open the overlay over the followed vehicle.
  await expect(page.getByTestId("inspector-panel")).toHaveCount(0);
  await page.screenshot({ path: `${SHOTS}/overlay-1-hud.png` });

  await page.getByTestId("hud-details").click();
  const overlay = page.getByTestId("inspector-panel");
  await expect(overlay).toBeVisible();
  for (const gone of ["tab-why", "tab-log", "tab-messages"]) await expect(page.getByTestId(gone)).toHaveCount(0);
  await expect(page.getByTestId("insp-health")).toBeVisible();
  await page.screenshot({ path: `${SHOTS}/overlay-2-overview.png` });

  await page.getByTestId("tab-sent").click();
  await expect(page.getByTestId("feed-row-sent").first()).toContainText("BSM", { timeout: 30_000 });
  // The rows hold still under the pointer, which is how a person aims at one while BSMs stream in.
  await page.getByTestId("feed-scroll-sent").hover();
  await expect(page.getByTestId("feed-status")).toContainText("held while the pointer is over the list");
  await page.getByTestId("feed-row-sent").first().click();
  await expect(page.getByTestId("feed-detail")).toBeVisible();
  await expect(page.getByTestId("feed-status")).toContainText("paused while you read");
  await expect(page.getByTestId("feed-hex")).toBeVisible();
  await page.screenshot({ path: `${SHOTS}/overlay-3-sent.png` });
  await page.getByTestId("feed-detail-close").click();
  await expect(page.getByTestId("feed-status")).toContainText(/live|held/);

  await page.getByTestId("tab-received").click();
  await expect(page.getByTestId("feed-row-received").first()).toBeVisible({ timeout: 30_000 });
  await expect(page.getByTestId("feed-row-received").first()).toContainText("BSM");
  await page.screenshot({ path: `${SHOTS}/overlay-4-received.png` });

  await page.getByTestId("inspector-close").click();
  await expect(overlay).toHaveCount(0);
  await expect(hud).toBeVisible();
});
