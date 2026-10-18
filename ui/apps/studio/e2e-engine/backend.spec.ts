/**
 * The PKI view against the real engine: the grid with the US SCMS switched on (WP-E2).
 *
 * The owner's ask: "All the SCMS entities have to be wired in and actually work and interact with
 * devices." The engine publishes the trust domain once a simulated second on `pki.state` and every
 * backend message on `pki.flow` (WP-E1); the Studio reads them through `inspect.trust`,
 * `inspect.flows` and `inspect.flow`. This checks that the PKI view draws every SCMS component of
 * the home provider as the engine reports it — the root and ICA offline, the rest online — that
 * devices reach the RA through the Location Obscurer Proxy and never directly, that a component's
 * card carries the engine's own counts, and that an SPDU opens in the decoder as the engine's tree.
 * It replaces the old Backend-diagram test: the diagram is now the CCMS's only.
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
    // A 20 s i-period is a compressed calendar, which the scenario must declare (WP-D2).
    .replace("duration_s: 60.0", "duration_s: 40.0\n  calendar: compressed")
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

test("the PKI view draws every SCMS component, its flows and an SPDU with the engine's data", async ({ page }) => {
  const shots = process.env.VWP_SHOT_DIR ?? tmpdir();
  await open(page);
  await page.getByTestId("speed").selectOption("1");
  await page.getByTestId("play").click();

  await page.getByTestId("backend-button").click();
  const panel = page.getByTestId("pki-panel");
  await expect(panel).toBeVisible();
  // The first snapshot is published a simulated second in.
  await expect(panel.getByTestId("pki-trust")).toBeVisible({ timeout: 60_000 });
  // The wire edition every PDU follows, as the engine reports it.
  await expect(panel.getByTestId("pki-time")).toContainText("ieee-1609.2-2022");

  const home = "provider-a";
  for (const role of ["root", "ica", "dcm", "eca", "lop", "ra", "la1", "la2", "aca", "ma", "crlg", "crl-store", "pg"]) {
    await expect(panel.getByTestId(`pki-comp-${home}/${role}/0`), `${role} is drawn`).toHaveCount(1);
  }
  await expect(panel.getByTestId("pki-comp-scms-manager/manager/0")).toHaveCount(1);
  // The Root CA and the ICA are kept offline, and the view says so; the RA is online.
  for (const role of ["root", "ica"]) await expect(panel.getByTestId(`pki-comp-${home}/${role}/0`)).toHaveAttribute("data-status", "offline");
  await expect(panel.getByTestId(`pki-comp-${home}/ra/0`)).toHaveAttribute("data-status", "online");
  // Some component carries traffic in the last two seconds once vehicles top up.
  await expect(panel.locator(".pki-comp.busy").first()).toBeVisible({ timeout: 60_000 });

  // A component's card: its counts as the engine reports them.
  const trust = await page.evaluate(async () => {
    const engine = window.__vwpStudio?.engine as unknown as {
      requestHttp(m: string, p: unknown, o: unknown): Promise<{ state: { providers: { components: { id: string; traffic: { sent: number } }[] }[] } }>;
    };
    return engine.requestHttp("inspect.trust", {}, { quiet: true });
  });
  const ra = trust.state.providers[0]?.components.find((c) => c.id === `${home}/ra/0`);
  expect(ra?.traffic.sent ?? 0).toBeGreaterThan(0);
  await panel.getByTestId(`pki-comp-${home}/ra/0`).click();
  const card = panel.getByTestId("pki-card");
  await expect(card).toContainText("Registration Authority");
  await expect(card.getByTestId("pki-card-status")).toContainText("online");
  await expect(card.getByTestId("pki-cert")).toBeVisible();
  await expect(card.getByTestId("pki-cert-trace").locator("li").first()).toBeVisible({ timeout: 20_000 });
  await page.screenshot({ path: join(shots, "pki-trust-engine.png") });

  // Devices reach the RA through the LOP, never directly: on every recorded flow.
  await panel.getByTestId("pki-tab-flows").click();
  await expect(panel.getByTestId("pki-flow-instances").locator("li").first()).toBeVisible({ timeout: 60_000 });
  const ends = await page.evaluate(async () => {
    const engine = window.__vwpStudio?.engine as unknown as {
      requestHttp(m: string, p: unknown, o: unknown): Promise<Record<string, unknown>>;
    };
    const list = (await engine.requestHttp("inspect.flows", { limit: 200 }, { quiet: true })) as { instances: { instance: string; flow: string }[] };
    const pairs: string[] = [];
    for (const i of list.instances.slice(0, 40)) {
      const d = (await engine.requestHttp("inspect.flow", { instance: i.instance }, { quiet: true })) as { records: { kind: string; from?: string; to?: string }[] };
      for (const r of d.records) if (r.kind === "msg") pairs.push(`${r.from?.split("/")[0] === "ee" ? "ee" : (r.from?.split("/")[1] ?? "")}>${r.to?.split("/")[0] === "ee" ? "ee" : (r.to?.split("/")[1] ?? "")}`);
    }
    return { flows: list.instances.map((i) => i.flow), pairs };
  });
  expect(ends.pairs).toContain("ee>lop");
  expect(ends.pairs).toContain("lop>ra");
  expect(ends.pairs).not.toContain("ee>ra");

  // A provisioning (or the newest) flow as a sequence diagram; an SPDU opens in the decoder.
  const kind = ends.flows.includes("provisioning") ? "provisioning" : "";
  if (kind) await panel.getByTestId("pki-flow-type").selectOption(kind);
  await panel.getByTestId("pki-flow-instances").locator("button").first().click();
  const seq = panel.getByTestId("pki-seq");
  await expect(seq).toBeVisible();
  await expect(seq.locator("g.pki-msg").first()).toBeVisible();
  await page.screenshot({ path: join(shots, "pki-flows-engine.png") });
  const withPdu = seq.locator("g.pki-msg.has-pdu").first();
  await withPdu.click();
  const msgCard = panel.getByTestId("pki-msg-card");
  if ((await panel.getByTestId("pdu-decoder").count()) === 0) await msgCard.locator("button.linklike").first().click();
  const dec = panel.getByTestId("pdu-decoder");
  await expect(dec).toBeVisible();
  await expect(dec.getByTestId("pdu-layer-0")).toHaveAttribute("data-type", "Ieee1609Dot2Data", { timeout: 20_000 });
  await expect(dec.getByTestId("pdu-tree-0").locator('[data-leaf="1"]').first()).toBeVisible();
  await page.screenshot({ path: join(shots, "pki-decoder-engine.png") });

  await panel.getByTestId("pki-close").click();
  await expect(panel).toHaveCount(0);
});
