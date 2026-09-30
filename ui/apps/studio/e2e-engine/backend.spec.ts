/**
 * The Backend view against the real engine: the grid with the US SCMS switched on.
 *
 * The owner's ask: "All the SCMS entities have to be wired in and actually work and interact with
 * devices." The engine publishes every entity's state once a simulated second on `backend.state`;
 * this checks that the page's Backend panel draws them as the engine reports them — every SCMS
 * authority present, the Root CA offline, the device-to-LOP path drawn and never device-to-RA —
 * and that clicking an entity shows the engine's own counters for it.
 */

import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { expect, test } from "@playwright/test";

import { EngineProcess, REPO, open } from "./support.js";

let engine: EngineProcess;

test.beforeAll(async () => {
  const source = readFileSync(join(REPO, "scenarios/phase1-grid.yaml"), "utf8");
  const dir = join(tmpdir(), `vwp-engine-e2e-backend-${process.pid}`);
  mkdirSync(dir, { recursive: true });
  const path = join(dir, "e2e-backend.yaml");
  const text = source
    .replace("name: phase1-grid", "name: e2e-backend")
    .replace("duration_s: 60.0", "duration_s: 40.0")
    .replace("rate_veh_per_h: 30.0", "rate_veh_per_h: 900.0")
    .replace("actors:\n", "actors:\n  backend:\n    protocol: protocol/scms/camp\n")
    // Cellular access, so vehicles top up through the LOP during the run.
    .replace("net:\n  layer: wsmp\n", "net:\n  layer: wsmp\n  uu:\n    id: cellular/uu/fixed-latency\n    params: {preset: 4g-east-coast}\n")
    .replace(
      "security:\n",
      [
        "security:",
        "  protocol:",
        "    id: protocol/scms/camp",
        "    params: {i_period_s: 20, cert_lifetime_s: 21, certs_per_period: 3, pool_periods: 2,",
        "             topup_below_periods: 1, cert_shuffle_window_s: 1, first_batch_delay_s: 1,",
        "             download_poll_interval_s: 1}",
        "",
      ].join("\n"),
    );
  expect(text).toContain("protocol/scms/camp");
  writeFileSync(path, text);
  engine = new EngineProcess(path);
  await engine.start();
});

test.afterAll(async () => {
  await engine?.stop();
});

test("the Backend view draws every SCMS entity with the engine's counts", async ({ page }) => {
  await open(page);
  await page.getByTestId("speed").selectOption("1");
  await page.getByTestId("play").click();

  await page.getByTestId("backend-button").click();
  const panel = page.getByTestId("backend-panel");
  await expect(panel).toBeVisible();
  // The first snapshot is published a simulated second in.
  await expect(panel.getByTestId("backend-diagram")).toBeVisible({ timeout: 60_000 });
  await expect(panel).toContainText("US SCMS");
  // The scheme every certificate and signed message uses, as the engine reports it.
  await expect(panel.getByTestId("backend-time")).toContainText("ecdsa-p256");

  for (const id of ["manager", "pg", "electors", "root", "ica", "dcm", "eca", "lop", "ra", "la1", "la2", "pca", "ma", "crlg", "crl-store", "ee"]) {
    await expect(panel.getByTestId(`backend-entity-${id}`), `${id} is drawn`).toHaveCount(1);
  }
  // The Root CA is kept offline, and the diagram says so.
  await expect(panel.getByTestId("backend-entity-root")).toHaveClass(/offline/);
  // Devices reach the RA through the Location Obscurer Proxy, never directly.
  await expect(panel.getByTestId("backend-edge-ra-lop")).toHaveCount(1, { timeout: 60_000 });
  await expect(panel.getByTestId("backend-edge-ee-ra")).toHaveCount(0);

  // An entity's own counters, as the engine reports them.
  await panel.getByTestId("backend-entity-pca").click();
  const side = panel.getByTestId("backend-side");
  await expect(side).toContainText("Pseudonym");
  const state = side.getByTestId("backend-entity-state");
  await expect(state).toContainText("certs issued");
  const issued = await page.evaluate(async () => {
    const engine = window.__vwpStudio?.engine as unknown as {
      requestHttp(m: string, p: unknown, o: unknown): Promise<{ state: { certs_issued?: number } }>;
    };
    return (await engine.requestHttp("inspect.entity", { entity: "pca" }, { quiet: true })).state.certs_issued ?? 0;
  });
  expect(issued).toBeGreaterThan(0);

  await page.screenshot({ path: process.env.VWP_SHOT ?? join(tmpdir(), "backend-view.jpg"), type: "jpeg", quality: 60 });
  await panel.getByTestId("backend-close").click();
  await expect(panel).toHaveCount(0);
});
