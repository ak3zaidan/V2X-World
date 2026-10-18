/**
 * WP-E2: the PKI view's Trust domain and Flows tabs in the real Studio, on fixtures.
 *
 * The mock server streams the run; the PKI queries it does not implement are answered on the page's
 * own socket from `test/fixtures/pki/pki-studio.json` — what `v2xw-server` answers on the WP-E1
 * fixtures (`crates/v2xw-server/tests/pki.rs` writes it and fails when it goes stale). Every other
 * request goes through to the mock untouched.
 */
import { expect, test, type Page } from "@playwright/test";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
type Bundle = Record<string, Record<string, unknown>>;
const studio = JSON.parse(readFileSync(join(here, "../test/fixtures/pki/pki-studio.json"), "utf8")) as { run: Bundle; sequence: Bundle };

const PKI = new Set(["inspect.trust", "inspect.flows", "inspect.flow", "inspect.pdu", "inspect.knowledge", "inspect.cert"]);

/** Answer one PKI request from the bundle, as the engine would (or as it would refuse). */
function answer(b: Bundle, method: string, params: Record<string, unknown>): { result?: unknown; error?: { code: number; message: string } } {
  const table = b[method] as Record<string, unknown> | undefined;
  const key =
    method === "inspect.trust"
      ? "newest"
      : method === "inspect.flows"
        ? String(params.flow ?? "")
        : method === "inspect.flow"
          ? String(params.instance)
          : method === "inspect.cert"
            ? String(params.hashedid8)
            : String(params.id ?? params.entity);
  let hit = table?.[key];
  if (hit !== undefined && (method === "inspect.pdu" || method === "inspect.knowledge")) hit = (hit as Record<string, unknown>)[String(params.view ?? "ground-truth")];
  if (method === "inspect.flows" && hit !== undefined && params.device) {
    const d = String(params.device).startsWith("ee/") ? String(params.device) : `ee/${String(params.device)}`;
    const h = hit as { instances: { devices: string[] }[] };
    const instances = h.instances.filter((i) => i.devices.includes(d));
    hit = { ...h, instances, total: instances.length };
  }
  return hit === undefined ? { error: { code: -32006, message: `unknown id ${key}` } } : { result: hit };
}

async function servePki(page: Page, b: Bundle, log: string[]): Promise<void> {
  await page.routeWebSocket(/.*/, (ws) => {
    const server = ws.connectToServer();
    ws.onMessage((m) => {
      if (typeof m === "string") {
        try {
          const req = JSON.parse(m) as { id?: number | string; method?: string; params?: Record<string, unknown> };
          if (req.method && PKI.has(req.method)) {
            log.push(`${req.method} ${JSON.stringify(req.params ?? {})}`);
            ws.send(JSON.stringify({ jsonrpc: "2.0", id: req.id, ...answer(b, req.method, req.params ?? {}) }));
            return;
          }
        } catch {
          // Not JSON: pass it on.
        }
      }
      server.send(m);
    });
  });
}

async function openPki(page: Page): Promise<void> {
  await page.goto("/");
  await expect(page.getByTestId("connection-state")).toHaveAttribute("data-state", "streaming", { timeout: 60_000 });
  await page.getByTestId("backend-button").click();
  await expect(page.getByTestId("pki-panel")).toBeVisible();
}

test("Trust domain: providers, offline chain, CTL, activity, component card and the view switch", async ({ page }) => {
  const log: string[] = [];
  await servePki(page, studio.run, log);
  await openPki(page);
  const panel = page.getByTestId("pki-panel");
  await expect(panel.getByTestId("pki-trust")).toBeVisible({ timeout: 30_000 });

  for (const p of ["provider-a", "provider-b"]) {
    const band = panel.getByTestId(`pki-provider-${p}`);
    await expect(band).toBeVisible();
    // The region is labelled an assumption.
    await expect(band.getByTestId(`pki-region-${p}`)).toContainText("assumed");
    // The offline chain: hatched, with a lock, and marked offline.
    for (const r of ["root", "ica"]) {
      const box = band.getByTestId(`pki-comp-${p}/${r}/0`);
      await expect(box).toHaveAttribute("data-status", "offline");
      await expect(box).toHaveClass(/status-offline/);
      await expect(box.locator(".pki-lock")).toHaveCount(1);
      const bg = await box.evaluate((el) => getComputedStyle(el).backgroundImage);
      expect(bg, `${p} ${r} is hatched`).toContain("repeating-linear-gradient");
    }
    const ra = band.getByTestId(`pki-comp-${p}/ra/0`);
    await expect(ra).toHaveAttribute("data-status", "online");
    expect(await ra.evaluate((el) => getComputedStyle(el).backgroundImage)).toBe("none");
  }
  // Electors are offline; the CTL line names its sequence and roots.
  await expect(panel.getByTestId("pki-comp-scms-manager/elector/0")).toHaveAttribute("data-status", "offline");
  await expect(panel.getByTestId("pki-ctl")).toContainText("CTL seq 0 · 2 roots");
  await panel.getByTestId("pki-ctl-history-toggle").click();
  await expect(panel.getByTestId("pki-ctl-history").locator("tbody tr")).toHaveCount(1);

  // Activity: the newest snapshot lights exactly the components the engine says.
  const newest = (studio.run["inspect.trust"] as Record<string, { activity: Record<string, unknown> }>).newest;
  const lit = Object.keys(newest?.activity ?? {});
  expect(lit.length).toBeGreaterThan(0);
  await expect(panel.locator(".pki-comp.busy")).toHaveCount(lit.length);
  for (const id of lit) await expect(panel.getByTestId(`pki-comp-${id}`)).toHaveClass(/busy/);

  // The component card: status, region, its certificate decoded with its trace.
  await panel.getByTestId("pki-comp-provider-a/aca/0").click();
  const card = panel.getByTestId("pki-card");
  await expect(card).toBeVisible();
  await expect(card.getByTestId("pki-card-status")).toContainText("online");
  await expect(card).toContainText("Ashburn, VA");
  await expect(card.getByTestId("pki-knowledge")).toBeVisible();
  await expect(card.getByTestId("pki-truth-join")).toBeVisible();

  // As LA1: the ACA's knowledge is withheld; the LA1's own card has no ground-truth join.
  await panel.getByTestId("pki-view").selectOption("provider-a/la1/0");
  await expect(card.getByTestId("pki-knowledge-withheld")).toBeVisible();
  const own = panel.locator(".pki-main").getByTestId("pki-knowledge");
  await expect(own).toContainText("What provider-a/la1/0 knows");
  await expect(own.getByTestId("pki-cannot-see").locator("li")).not.toHaveCount(0);
  await expect(own.getByTestId("pki-truth-join")).toHaveCount(0);
  expect(log.some((l) => l.startsWith("inspect.knowledge") && l.includes('"view":"provider-a/la1/0"'))).toBe(true);
  await page.screenshot({ path: process.env.VWP_SHOT_DIR ? join(process.env.VWP_SHOT_DIR, "pki-trust-fixture.png") : join(here, "../test-results/pki-trust-fixture.png") });
});

test("Flows: a provisioning run as a sequence diagram, the shuffle wait, the decoder and the view rule", async ({ page }) => {
  const log: string[] = [];
  await servePki(page, studio.sequence, log);
  await openPki(page);
  const panel = page.getByTestId("pki-panel");
  await panel.getByTestId("pki-tab-flows").click();
  await expect(panel.getByTestId("pki-flows")).toBeVisible();

  await panel.getByTestId("pki-flow-type").selectOption("provisioning");
  await expect(panel.getByTestId("pki-flow-instances").locator("li")).toHaveCount(2);
  await panel.getByTestId("pki-flow-provider-a/provisioning/2").click();
  const seq = panel.getByTestId("pki-seq");
  await expect(seq).toBeVisible();
  for (const lane of ["ee/1000", "provider-a/lop/0", "provider-a/ra/0", "provider-a/la1/0", "provider-a/la2/0", "provider-a/aca/0"]) {
    await expect(seq.getByTestId(`pki-lane-${lane}`)).toHaveCount(1);
  }
  await expect(seq.locator("g.pki-msg")).toHaveCount(18);
  await expect(seq.getByTestId("pki-gap-shuffle")).toHaveCount(1);
  await expect(seq.getByTestId("pki-gap-shuffle")).toContainText("shuffle wait 60.0 s");
  expect(await seq.getByTestId("pki-gap-shuffle").locator("rect").getAttribute("fill")).toBe("url(#pki-hatch)");
  await expect(panel.getByTestId("pki-stages").locator("tbody tr")).toHaveCount(10);

  // Click the device's request: one SPDU, so the decoder opens on it, with the hex and the tree.
  await seq.getByTestId("pki-msg-provider-a/m/6").click();
  const dec = panel.getByTestId("pdu-decoder");
  await expect(dec).toBeVisible();
  await expect(dec).toContainText("EeRaCertRequestSpdu");
  await expect(dec.getByTestId("pdu-layer-0")).toHaveAttribute("data-type", "Ieee1609Dot2Data");
  // The tree shows each JER leaf of the engine's answer, and the plaintext the RA opened.
  const gt = (studio.sequence["inspect.pdu"] as Record<string, Record<string, { tree: { layers: { jer: unknown }[] } }>>)["provider-a/pdu/3"]?.["ground-truth"];
  const leaves = (v: unknown): number => (Array.isArray(v) ? v.reduce<number>((n, x) => n + leaves(x), 0) : typeof v === "object" && v !== null ? Object.values(v).reduce<number>((n, x) => n + leaves(x), 0) : 1);
  await expect(dec.getByTestId("pdu-tree-0").locator('[data-leaf="1"]')).toHaveCount(leaves(gt?.tree.layers[0]?.jer));
  await expect(dec.getByTestId("pdu-plaintext")).toBeVisible();
  await page.screenshot({ path: process.env.VWP_SHOT_DIR ? join(process.env.VWP_SHOT_DIR, "pki-flows-fixture.png") : join(here, "../test-results/pki-flows-fixture.png") });

  // As the ACA: it never saw the device's request; it saw the RA's cert-request and cannot read
  // the RA's plaintext.
  await panel.getByTestId("pki-view").selectOption("provider-a/aca/0");
  await expect(dec).toHaveCount(0);
  await expect(seq.getByTestId("pki-msg-provider-a/m/6")).toHaveAttribute("data-observed", "0");
  await expect(seq.locator('g.pki-msg[data-observed="1"]')).toHaveCount(2);
  // As the RA: the LOP's forward of the request reached it, and it opened the plaintext itself.
  await panel.getByTestId("pki-view").selectOption("provider-a/ra/0");
  await seq.getByTestId("pki-msg-provider-a/m/9").click();
  await expect(dec).toBeVisible();
  await expect(dec.getByTestId("pdu-view")).toHaveText("provider-a/ra/0");
  await expect(dec.getByTestId("pdu-plaintext")).toBeVisible();
  // The LA1 is shown the PLV it built, sealed to the ACA: ciphertext, plaintext withheld.
  await panel.getByTestId("pki-view").selectOption("provider-a/la1/0");
  const plv = seq.locator("g.pki-msg", { hasText: "EncryptedIndividualPLV" }).first();
  await plv.click();
  await panel.getByTestId("pki-msg-card").locator("button.linklike").first().click();
  await expect(dec).toBeVisible();
  await expect(dec.getByTestId("pdu-plaintext-hidden")).toContainText("provider-a/aca/0");
  await expect(dec.getByTestId("pdu-plaintext")).toHaveCount(0);
});
