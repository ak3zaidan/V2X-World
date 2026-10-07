/**
 * The agent panel against the mock engine, which has no agent: the header's Agent button opens
 * the panel, and the panel says plainly that this engine has no agent and how to get one, rather
 * than offering a prompt box that cannot work. (The panel against `v2xw serve` is driven in the
 * engine suite's agent check.)
 */

import { expect, test } from "@playwright/test";

test("the Agent button opens the agent panel, which explains an engine without an agent", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("agent-button")).toBeVisible();
  await page.getByTestId("agent-button").click();
  await expect(page.getByTestId("agent-panel")).toBeVisible();
  await expect(page.getByTestId("agent-unavailable")).toContainText("v2xw serve");
  // No model, no agent: the prompt box is disabled rather than silently doing nothing.
  await expect(page.getByTestId("agent-input")).toBeDisabled();
  // Closing and reopening keeps the panel working (its state lives outside the sheet).
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("agent-panel")).toHaveCount(0);
  await page.getByTestId("agent-button").click();
  await expect(page.getByTestId("agent-panel")).toBeVisible();
});
