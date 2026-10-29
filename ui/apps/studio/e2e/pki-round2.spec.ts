/**
 * The certificate views after the second fresh-eyes review, in the real Studio at 1280 × 800:
 * one name per vehicle, the engine's zero-lifetime enrolment certificate shown as a simulator
 * limit, a revoked vehicle's story (whose list, how many refuse it, its case), the revocation
 * table's next list and who each list revokes, a case's beats at their times, the Messages tab
 * opening on one example of each kind, and the certificate rings chosen from the map's Layers menu.
 *
 * As in `pki-polish.spec.ts`, the PKI queries the mock does not implement are answered on the
 * page's socket from `test/fixtures/pki/pki-studio.json`; `inspect.pki` comes from the mock's
 * recorded tabs (`VWP_PKI_TABS`).
 */
import { expect, test, type Page } from "@playwright/test";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
type Bundle = Record<string, Record<string, unknown>>;
const studio = JSON.parse(readFileSync(join(here, "../test/fixtures/pki/pki-studio.json"), "utf8")) as { run: Bundle; sequence: Bundle };
type Vehicle = { node: number; device: string; enrolment: string; truth?: { revoked_certs: number } };
const tabs = JSON.parse(readFileSync(join(here, "../test/fixtures/pki/pki-tabs.json"), "utf8")) as {
  vehicles: { vehicles: Vehicle[] };
  cases: Record<string, { cases: { case: string; truth?: { accused: { device: string }[] } }[] }>;
};

/** Answer the PKI queries the mock does not implement from a bundle, and `inspect.cert` from `certs`. */
async function serve(page: Page, b: Bundle, certs: Record<string, unknown> = {}): Promise<void> {
  await page.routeWebSocket(/.*/, (ws) => {
    const server = ws.connectToServer();
    ws.onMessage((m) => {
      if (typeof m === "string") {
        try {
          const req = JSON.parse(m) as { id?: number | string; method?: string; params?: Record<string, unknown> };
          const p = req.params ?? {};
          const table = req.method ? (b[req.method] as Record<string, unknown> | undefined) : undefined;
          let hit: unknown;
          if (req.method === "inspect.trust") hit = table?.newest;
          else if (req.method === "inspect.flows") hit = table?.[String(p.flow ?? "")];
          else if (req.method === "inspect.flow") hit = table?.[String(p.instance)];
          else if (req.method === "inspect.pdu" || req.method === "inspect.knowledge") {
            const h = table?.[String(p.id ?? p.entity)] as Record<string, unknown> | undefined;
            hit = h?.[String(p.view ?? "ground-truth")];
          } else if (req.method === "inspect.cert") hit = certs[String(p.hashedid8)];
          if (req.method && ["inspect.trust", "inspect.flows", "inspect.flow", "inspect.pdu", "inspect.knowledge", "inspect.cert"].includes(req.method)) {
            ws.send(JSON.stringify({ jsonrpc: "2.0", id: req.id, ...(hit === undefined ? { error: { code: -32006, message: "unknown certificate" } } : { result: hit }) }));
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

async function open(page: Page, tab: "trust" | "vehicles" | "cases" | "crls" | "flows"): Promise<void> {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await expect(page.getByTestId("connection-state")).toHaveAttribute("data-state", "streaming", { timeout: 60_000 });
  await page.getByTestId("backend-button").click();
  await page.getByTestId(`pki-tab-${tab}`).click();
}

/** An enrolment certificate as `v2xw-server` writes one for a lifetime under an hour: zero hours. */
function zeroLifetimeEc(id8: string): Record<string, unknown> {
  const NOW = 731232071;
  const rules = ["well-formed", "chain-to-root", "root-on-ctl", "signatures", "issuer-permissions", "chain-length", "validity", "region", "crl"];
  return {
    id8,
    chain: [id8],
    valid: false,
    time32: NOW,
    t_ns: 66e9,
    tolerated: [],
    trace: rules.map((rule) => ({ rule, status: rule === "validity" ? "fail" : "pass", detail: rule === "validity" ? ["cert 0: not valid at the policy time"] : [] })),
    tree: { issuer: { self: "sha256" }, toBeSigned: { id: { name: "ee" }, validityPeriod: { duration: { hours: 0 }, start: NOW - 240 } }, type: "explicit", version: 3 },
  };
}

test("one name per vehicle: its map number, its device id beside it, and either finds it", async ({ page }) => {
  await serve(page, studio.run);
  await open(page, "vehicles");
  const panel = page.getByTestId("pki-panel");
  const row = panel.locator('[data-testid="pki-vehicle-row"][data-node="19"]');
  await expect(row.getByTestId("pki-vehicle-name")).toContainText("Vehicle 19");
  await expect(row.getByTestId("pki-vehicle-name")).toContainText("device 1000019");
  const rows = panel.getByTestId("pki-vehicle-row");
  // The map's number finds it, and so does the device id the records use.
  for (const q of ["19", "1000019", "Vehicle 19"]) {
    await panel.getByTestId("pki-vehicle-find").fill(q);
    await expect(rows).toHaveCount(1);
    await expect(rows.first()).toHaveAttribute("data-node", "19");
  }
  await panel.getByTestId("pki-vehicle-find").fill("");
  await expect(rows).toHaveCount(tabs.vehicles.vehicles.length);
  // The accused of a case reads the same.
  await panel.getByTestId("pki-tab-cases").click();
  const accused = tabs.cases["ground-truth"].cases[0].truth!.accused[0].device;
  const node = tabs.vehicles.vehicles.find((v) => v.device === accused)!.node;
  await panel.locator(`[data-testid="pki-case-card"][data-case="${tabs.cases["ground-truth"].cases[0].case}"]`).click();
  await expect(panel.getByTestId("pki-case-accused")).toContainText(`Vehicle ${node}`);
});

test("a zero-lifetime enrolment certificate is a simulator limit, not a failing vehicle", async ({ page }) => {
  const v = tabs.vehicles.vehicles.find((x) => x.node === 19)!;
  await serve(page, studio.run, { [v.enrolment]: zeroLifetimeEc(v.enrolment) });
  await open(page, "vehicles");
  const panel = page.getByTestId("pki-panel");
  const row = panel.locator('[data-testid="pki-vehicle-row"][data-node="19"]');
  await row.click();
  await expect(panel.getByTestId("pki-glance-ec-limit")).toBeVisible();
  await expect(panel.getByTestId("pki-glance-ec-tile")).toHaveAttribute("data-limit", "1");
  await expect(panel.getByTestId("pki-glance-ec-says")).toContainText("lifetime not modelled in this scenario");
  // It decides nothing: the row and the card keep the vehicle's own state.
  await expect(row).toHaveAttribute("data-state", "healthy");
  await expect(panel.getByTestId("pki-vehicle-card-state")).toHaveAttribute("data-state", "healthy");
});

test("a revoked vehicle: whose list names it, how many refuse it, its case; its certificates held but refused", async ({ page }) => {
  await serve(page, studio.run);
  await open(page, "vehicles");
  const panel = page.getByTestId("pki-panel");
  const revoked = tabs.vehicles.vehicles.find((x) => (x.truth?.revoked_certs ?? 0) > 0)!;
  const row = panel.locator(`[data-testid="pki-vehicle-row"][data-node="${revoked.node}"]`);
  await expect(row.getByTestId("pki-vehicle-pool")).toHaveAttribute("data-revoked", "1");
  await expect(row.getByTestId("pki-vehicle-pool")).toContainText("held · refused");
  await row.click();
  const by = panel.getByTestId("pki-glance-revoked-by");
  await expect(by).toContainText("on provider-a's list v2");
  await expect(panel.getByTestId("pki-glance-revocation")).not.toContainText("not yet heard");
  // Its own list, as a receiver, is a line of its own (the fixture's vehicles report no list installed).
  await expect(panel.getByTestId("pki-glance-own-list")).toHaveAttribute("title", /As a receiver/);
  await expect(panel.getByTestId("pki-glance-own-list")).toContainText(/Its own copy of the list|It has not installed a revocation list yet/);
  await expect(panel.getByTestId("pki-glance-pool")).toContainText("held · refused");
  const theCase = tabs.cases["ground-truth"].cases.find((c) => c.truth?.accused.some((a) => a.device === revoked.device))!;
  await by.getByTestId("pki-glance-case").click();
  await expect(panel.getByTestId("pki-tab-cases")).toHaveAttribute("aria-selected", "true");
  await expect(panel.locator(`[data-testid="pki-case-card"][data-case="${theCase.case}"]`)).toHaveAttribute("aria-pressed", "true");
});

test("Revocation: the next list by the scenario's schedule, and who each list revokes one click away", async ({ page }) => {
  await serve(page, studio.run);
  await open(page, "crls");
  const panel = page.getByTestId("pki-panel");
  const sla = panel.getByTestId("pki-crl-sla");
  await expect(sla.first()).toBeVisible();
  // The fixture's generators publish on a schedule: the next list is said by it, the nextCrl deadline sits in the tooltip.
  await expect(sla.first()).toHaveAttribute("data-basis", "schedule");
  await expect(sla.first()).toContainText(/within .* \(every /);
  await expect(sla.first()).toHaveAttribute("title", /nextCrl/);
  await panel.getByTestId("pki-crl-entries-link").first().click();
  const card = panel.getByTestId("pki-crl-card");
  await expect(card.getByTestId("pki-crl-matches").first()).toContainText(/Vehicle \d+/);
  await card.getByTestId("pki-crl-case").first().click();
  await expect(panel.getByTestId("pki-tab-cases")).toHaveAttribute("aria-selected", "true");
});

test("a case's beats at their simulated times, the step each took, and privacy folded", async ({ page }) => {
  await serve(page, studio.run);
  await open(page, "cases");
  const panel = page.getByTestId("pki-panel");
  const id = tabs.cases["ground-truth"].cases[0].case;
  await panel.locator(`[data-testid="pki-case-card"][data-case="${id}"]`).click();
  const story = panel.getByTestId("pki-case-story");
  await expect(story.locator('li[data-beat="reports-in"]').getByTestId("pki-story-at")).toContainText(/^at \d+\.\d s$/);
  await expect(story.locator('li[data-beat="reports-in"]').getByTestId("pki-story-step")).toHaveText("the first report");
  await expect(story.locator('li[data-beat="crl-published"]').getByTestId("pki-story-step")).toContainText(/after the step before|at once/);
  // Who learns what is there, folded behind one line per step.
  const fold = panel.getByTestId("pki-case-learned-toggle").first();
  await expect(fold).toContainText(/Privacy: who learns what \(\d+ facts?\)/);
  await expect(panel.getByTestId("pki-case-learned").first()).toBeHidden();
  await fold.click();
  await expect(panel.getByTestId("pki-case-learned").first()).toBeVisible();
  await expect(panel.getByTestId("pki-case-detail")).not.toContainText("NOT");
});

test("Messages open on one example of each kind, with the reports folded to a count", async ({ page }) => {
  await serve(page, studio.sequence);
  await open(page, "flows");
  const panel = page.getByTestId("pki-panel");
  const examples = panel.getByTestId("pki-flow-examples");
  await expect(examples.locator("li")).toHaveCount(5);
  // The first example is a certificate download, and it is the flow drawn.
  await expect(examples.locator("li").first().getByRole("button")).toHaveAttribute("data-testid", "pki-flow-example-provisioning");
  await expect(panel.locator(".pki-flow-title")).toHaveAttribute("title", /\/provisioning\//);
  const list = panel.getByTestId("pki-flow-instances");
  for (const t of await list.locator("button").evaluateAll((els) => els.map((e) => e.getAttribute("data-testid") ?? ""))) expect(t).not.toContain("/report/");
  await panel.getByTestId("pki-flow-reports-folded").click();
  await expect(panel.getByTestId("pki-flow-type")).toHaveValue("report");
  await expect(list.locator("li")).toHaveCount(2);
});

test("the certificate rings are chosen from the map's Layers menu, and the legend matches the badges", async ({ page }) => {
  await serve(page, studio.run);
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await expect(page.getByTestId("connection-state")).toHaveAttribute("data-state", "streaming", { timeout: 60_000 });
  await page.getByTestId("overlays-button").click();
  await page.getByTestId("layers-pki-map-mode").selectOption("credential");
  const legend = page.getByTestId("pki-map-legend");
  await expect(legend).toHaveAttribute("data-mode", "credential");
  // Revoked is a double ring in the badges' error colour; top-up due is the accent blue, not an orange like starving.
  await expect(legend.locator('[data-state="revoked"] i')).toHaveClass(/double/);
  const ring = async (s: string): Promise<string> => legend.locator(`[data-state="${s}"] i`).evaluate((e) => getComputedStyle(e).borderTopColor);
  expect(await ring("topup-due")).not.toBe(await ring("starving"));
  expect(await ring("revoked")).not.toBe(await ring("untrusted"));
});

test("the empty inspector leads to the certificates of the run", async ({ page }) => {
  await serve(page, studio.run);
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await expect(page.getByTestId("connection-state")).toHaveAttribute("data-state", "streaming", { timeout: 60_000 });
  await page.getByTestId("inspector-toggle").click();
  await page.getByTestId("inspector-empty-certificates").click();
  await expect(page.getByTestId("pki-panel")).toBeVisible();
  await expect(page.getByTestId("pki-tab-vehicles")).toHaveAttribute("aria-selected", "true");
});
