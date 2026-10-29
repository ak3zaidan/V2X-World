/**
 * The certificate views in the real Studio: the certificate card (kind, validity, permissions, the
 * chain named link by link, the receiver's checks, the reason it fails), the drawer that opens any
 * certificate from anywhere, a vehicle's certificates at a glance, the case story and the trust
 * chain.
 *
 * The mock server streams the run and answers `inspect.pki` from the recorded tabs
 * (`VWP_PKI_TABS`). `inspect.trust` is answered on the page's socket from the WP-E1 bundle
 * (`test/fixtures/pki/pki-studio.json`), and `inspect.cert` with certificates built here on those
 * components' own HashedId8s, in the shape `v2xw-server` answers (`scenarios/credential-
 * lifecycle.yaml`, 2026-10-28): every other request goes through to the mock untouched.
 */
import { expect, test, type Page } from "@playwright/test";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
type Json = Record<string, unknown>;
const studio = JSON.parse(readFileSync(join(here, "../test/fixtures/pki/pki-studio.json"), "utf8")) as { run: Record<string, Record<string, Json>> };
const tabs = JSON.parse(readFileSync(join(here, "../test/fixtures/pki/pki-tabs.json"), "utf8")) as {
  vehicle: Record<string, { active: string; grid: { held: { i: number; j: number; id8?: string; state: string }[] } }>;
  cases: Record<string, { cases: { case: string; stages: { stage: string }[] }[] }>;
};
const trust = studio.run["inspect.trust"].newest as { state: { providers: { label: string; components: { id: string; role: string; cert: string | null }[] }[] } };
const compA = Object.fromEntries(trust.state.providers[0].components.map((c) => [c.role, c.cert as string]));
const ROOT = compA.root;
const ICA = compA.ica;
const ACA = compA.aca;

const RULES = ["well-formed", "chain-to-root", "root-on-ctl", "signatures", "issuer-permissions", "chain-length", "validity", "region", "crl"];
const NOW = 731232071;

/** A certificate answer in `inspect.cert`'s shape; `fail` names the rule that fails. */
function cert(id8: string, chain: string[], tbs: Json, fail: string | null = null): Json {
  return {
    id8,
    chain,
    valid: fail === null,
    time32: NOW,
    t_ns: 66e9,
    tolerated: [],
    trace: RULES.map((rule) => ({ rule, status: rule === fail ? "fail" : "pass", detail: rule === fail ? ["cert 0: not valid at the policy time"] : [] })),
    tree: { issuer: chain.length > 1 ? { sha256AndDigest: chain[1].toUpperCase() } : { self: "sha256" }, toBeSigned: tbs, type: "explicit", version: 3 },
  };
}
const US = { identifiedRegion: [{ countryOnly: 840 }, { countryOnly: 124 }, { countryOnly: 484 }] };
const authority = (name: string, psid: number): Json => ({
  appPermissions: [{ psid }],
  certIssuePermissions: [{ subjectPermissions: { all: null } }],
  id: { name },
  region: US,
  validityPeriod: { duration: { years: 30 }, start: 699675053 },
  crlSeries: 2,
  cracaId: "1D66DE",
});
const pseudonym = (i: number): Json => ({
  appPermissions: [{ psid: 32 }, { psid: 38 }],
  id: { linkageData: { iCert: i, "linkage-value": "21AD34291C7B41479C" } },
  validityPeriod: { duration: { minutes: 10140 }, start: NOW - 60 },
  crlSeries: 1,
  cracaId: "1D66DE",
});

const vehicle = tabs.vehicle["ee/1000019"];
const valid2 = vehicle.grid.held.find((c) => c.state === "valid" && c.id8)!;
const CERTS: Record<string, Json> = {
  [ACA]: cert(ACA, [ACA, ICA, ROOT], authority("aca.provider-a", 35)),
  [ICA]: cert(ICA, [ICA, ROOT], authority("ica.provider-a", 35)),
  [ROOT]: cert(ROOT, [ROOT], authority("root.provider-a", 35)),
  [vehicle.active]: cert(vehicle.active, [vehicle.active, ACA, ICA, ROOT], pseudonym(634)),
  // A certificate of a later week, checked now: the receiver's validity step fails.
  [valid2.id8 as string]: cert(valid2.id8 as string, [valid2.id8 as string, ACA, ICA, ROOT], pseudonym(634), "validity"),
};

async function serve(page: Page, certs: Record<string, Json>): Promise<void> {
  await page.routeWebSocket(/.*/, (ws) => {
    const server = ws.connectToServer();
    ws.onMessage((m) => {
      if (typeof m === "string") {
        try {
          const req = JSON.parse(m) as { id?: number | string; method?: string; params?: Record<string, unknown> };
          if (req.method === "inspect.trust") {
            ws.send(JSON.stringify({ jsonrpc: "2.0", id: req.id, result: trust }));
            return;
          }
          if (req.method === "inspect.cert") {
            const hit = certs[String(req.params?.hashedid8)];
            ws.send(JSON.stringify({ jsonrpc: "2.0", id: req.id, ...(hit ? { result: hit } : { error: { code: -32006, message: "unknown certificate" } }) }));
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

async function open(page: Page, tab: "trust" | "vehicles" | "cases"): Promise<void> {
  await page.goto("/");
  await expect(page.getByTestId("connection-state")).toHaveAttribute("data-state", "streaming", { timeout: 60_000 });
  await page.getByTestId("backend-button").click();
  await page.getByTestId(`pki-tab-${tab}`).click();
}

test("the certificate card: kind, permissions, the chain named to a trusted root, the checks, and any link opens its certificate", async ({ page }) => {
  await serve(page, CERTS);
  await open(page, "trust");
  const panel = page.getByTestId("pki-panel");
  await panel.getByTestId("pki-comp-provider-a/aca/0").click();
  const card = panel.getByTestId("pki-card").getByTestId("pki-cert");
  await expect(card).toHaveAttribute("data-status", "valid");
  await expect(card.getByTestId("pki-cert-status")).toContainText("Valid");
  await expect(card).toContainText("Pseudonym CA certificate (ACA)");
  await expect(card.getByTestId("pki-cert-psids")).toContainText("SCMS security management · PSID 35");
  await expect(card).toContainText("Canada, Mexico, United States");
  await expect(card.getByTestId("pki-cert-validity")).toBeVisible();
  // The chain, leaf to root, each link named by its holder; the root is on the trust list.
  const chain = card.getByTestId("pki-cert-chain").locator("li");
  await expect(chain).toHaveCount(3);
  await expect(chain.nth(1)).toContainText("Intermediate");
  await expect(chain.nth(2)).toContainText("on the trust list");
  // Every check passed, so the list is one click away; opened, it has a line per rule.
  await expect(card.getByTestId("pki-cert-trace")).toHaveCount(0);
  await card.getByTestId("pki-cert-checks-toggle").click();
  await expect(card.getByTestId("pki-cert-trace").locator("li")).toHaveCount(RULES.length);
  await expect(card.getByTestId("pki-cert-trace").locator("li.st-pass")).toHaveCount(RULES.length);
  // The raw certificate is behind its toggle.
  await expect(card.getByTestId("pki-cert-tree")).toHaveCount(0);
  await card.getByTestId("pki-cert-raw-toggle").click();
  await expect(card.getByTestId("pki-cert-tree")).toBeVisible();
  // The issuer opens its own certificate over the card; Escape closes that first, then the card.
  await card.getByTestId("pki-cert-issuer").click();
  const drawer = panel.getByTestId("pki-cert-drawer");
  await expect(drawer.getByTestId("pki-cert-drawer-card")).toHaveAttribute("data-status", "valid");
  await expect(drawer).toContainText("Intermediate CA certificate (ICA)");
  await page.keyboard.press("Escape");
  await expect(drawer).toHaveCount(0);
  await expect(panel.getByTestId("pki-card")).toBeVisible();
  await page.screenshot({ path: process.env.VWP_SHOT_DIR ? join(process.env.VWP_SHOT_DIR, "pki-cert-card.png") : join(here, "../test-results/pki-cert-card.png") });
});

test("a vehicle at a glance; a certificate that fails says why, from the grid", async ({ page }) => {
  await serve(page, CERTS);
  await open(page, "vehicles");
  const panel = page.getByTestId("pki-panel");
  // The rows are reachable from the keyboard.
  const row = panel.locator('[data-testid="pki-vehicle-row"][data-node="19"]');
  await row.focus();
  await page.keyboard.press("Enter");
  await expect(row).toHaveAttribute("aria-selected", "true");
  const glance = panel.getByTestId("pki-vehicle-glance");
  await expect(glance.getByTestId("pki-glance-pc")).toContainText(vehicle.active.slice(0, 4));
  await expect(glance).toContainText("week i = 634, certificate j = 0");
  await expect(glance).toContainText("not revoked");
  // The certificate it signs with: valid, a pseudonym certificate, its chain to the root.
  const active = panel.getByTestId("pki-vehicle-card").getByTestId("pki-vehicle-cert");
  await expect(active).toHaveAttribute("data-status", "valid");
  await expect(active).toContainText("Pseudonym certificate (PC)");
  await expect(active.getByTestId("pki-cert-chain").locator("li")).toHaveCount(4);
  await expect(active.getByTestId("pki-cert-psids")).toContainText("Basic safety messages (BSM)");
  // A cell of the grid opens that certificate; this one fails its validity check, and says so.
  await panel.locator(`[data-testid="pki-grid"] rect[data-i="${valid2.i}"][data-j="${valid2.j}"]`).click();
  const card = panel.getByTestId("pki-cert-drawer-card");
  await expect(card).toHaveAttribute("data-status", "expired");
  await expect(card.getByTestId("pki-cert-why")).toContainText("not valid at the policy time");
  // A failure opens the checks without a click, the failing rule marked.
  await expect(card.getByTestId("pki-cert-trace").locator('li.st-fail[data-rule="validity"]')).toHaveCount(1);
  await page.screenshot({ path: process.env.VWP_SHOT_DIR ? join(process.env.VWP_SHOT_DIR, "pki-vehicle-cert.png") : join(here, "../test-results/pki-vehicle-cert.png") });
});

test("a case reads as four beats, report to refusal", async ({ page }) => {
  await serve(page, CERTS);
  await open(page, "cases");
  const panel = page.getByTestId("pki-panel");
  const gt = tabs.cases["ground-truth"].cases;
  for (const c of gt) {
    await panel.locator(`[data-testid="pki-case-card"][data-case="${c.case}"]`).click();
    const story = panel.getByTestId("pki-case-story");
    await expect(story.locator("li")).toHaveCount(4);
    const reached = new Set(c.stages.map((s) => s.stage));
    for (const beat of ["reports-in", "crl-published", "enforced"]) {
      await expect(story.locator(`li[data-beat="${beat}"]`), `${c.case} ${beat}`).toHaveClass(reached.has(beat) ? /done/ : /todo/);
    }
  }
});

test("the trust chain: every provider's root to its authorities, each opening its card", async ({ page }) => {
  await serve(page, CERTS);
  await open(page, "trust");
  const panel = page.getByTestId("pki-panel");
  const tc = panel.getByTestId("pki-trustchain");
  await expect(tc).toBeVisible();
  for (const p of trust.state.providers) await expect(tc.getByTestId(`pki-tc-provider-${p.label}`)).toBeVisible();
  await tc.getByTestId("pki-tc-provider-a/la1/0").click();
  await expect(panel.getByTestId("pki-card").getByTestId("pki-card-does")).toContainText("pre-linkage values");
  // The health strip says what each number means.
  await expect(panel.getByTestId("pki-health-pending")).toHaveAttribute("data-tone", /ok|warn/);
  await expect(panel.getByTestId("pki-health-crl")).toContainText("since the newest revocation list");
});
