/**
 * The certificate views after the fresh-eyes review, in the real Studio at 1280 × 800: vehicles
 * sorted problems-first with filter chips and no column of copies, the header on one row with the
 * view picker in words, the legend above the diagram, activity in a neutral tone, a case's four
 * beats on its list card, a message that reads as a sentence and holds its flow while the list
 * grows, and the revocation schedule said once.
 *
 * As in `pki.spec.ts`, the PKI queries the mock does not implement are answered on the page's own
 * socket from `test/fixtures/pki/pki-studio.json`; `inspect.pki` comes from the mock's recorded
 * tabs (`VWP_PKI_TABS`).
 */
import { expect, test, type Page } from "@playwright/test";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
type Bundle = Record<string, Record<string, unknown>>;
const studio = JSON.parse(readFileSync(join(here, "../test/fixtures/pki/pki-studio.json"), "utf8")) as { run: Bundle; sequence: Bundle };

/** The triage order the Vehicles tab sorts by (`lib/pkiRecords.ts` CREDENTIAL_ORDER). */
const ORDER = ["revoked", "untrusted", "starving", "crl-overdue", "topup-due", "between", "healthy", "left"];
const NEWER = "provider-a/provisioning/999";

/**
 * Answer the PKI queries from a bundle. After `grow()`, every `inspect.flows` answer carries one
 * flow newer than the rest, as a live run's list does.
 */
async function serve(page: Page, b: Bundle, certs: Record<string, unknown> = {}): Promise<{ grow: () => void }> {
  let growing = false;
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
          else if (req.method === "inspect.flows") {
            const h = table?.[String(p.flow ?? "")] as { instances: { instance: string; first_t: number }[]; total: number } | undefined;
            if (h && growing) {
              const top = h.instances[0];
              hit = { ...h, instances: [{ ...top, instance: NEWER, first_t: Math.max(...h.instances.map((i) => i.first_t)) + 1e9 }, ...h.instances], total: h.total + 1 };
            } else hit = h;
          } else if (req.method === "inspect.flow") {
            const id = String(p.instance);
            const h = table?.[id === NEWER ? String(Object.keys(table ?? {}).find((k) => k.includes("/provisioning/"))) : id] as Record<string, unknown> | undefined;
            hit = h && id === NEWER ? { ...h, instance: NEWER } : h;
          } else if (req.method === "inspect.pdu" || req.method === "inspect.knowledge") {
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
  return { grow: () => (growing = true) };
}

async function open(page: Page, tab: "trust" | "vehicles" | "cases" | "crls" | "flows"): Promise<void> {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await expect(page.getByTestId("connection-state")).toHaveAttribute("data-state", "streaming", { timeout: 60_000 });
  await page.getByTestId("backend-button").click();
  await page.getByTestId(`pki-tab-${tab}`).click();
}

test("Vehicles: problems first, chips that filter, constant columns said once, and no sideways scroll at 1280", async ({ page }) => {
  await serve(page, studio.run);
  await open(page, "vehicles");
  const panel = page.getByTestId("pki-panel");
  const rows = panel.getByTestId("pki-vehicle-row");
  await expect(rows.first()).toBeVisible();
  const states = await rows.evaluateAll((els) => els.map((e) => e.getAttribute("data-state") ?? ""));
  expect(states.length).toBeGreaterThan(2);
  // The fixture has two vehicles a published list revokes and others that are fine: both kinds, problems on top.
  expect(states).toContain("revoked");
  expect(new Set(states).size).toBeGreaterThan(1);
  const ranks = states.map((s) => ORDER.indexOf(s));
  expect(ranks).toEqual([...ranks].sort((a, b) => a - b));
  // A chip shows only its vehicles; "All" brings every row back.
  const chip = panel.locator('[data-testid="pki-chip"][data-state="revoked"]');
  await chip.click();
  await expect(chip).toHaveAttribute("aria-pressed", "true");
  await expect(rows).toHaveCount(states.filter((s) => s === "revoked").length);
  for (const s of await rows.evaluateAll((els) => els.map((e) => e.getAttribute("data-state")))) expect(s).toBe("revoked");
  await panel.getByTestId("pki-chip-all").click();
  await expect(rows).toHaveCount(states.length);
  // A value every vehicle shares is one sentence, not a column.
  await expect(panel.getByTestId("pki-vehicles-same")).toContainText("Every vehicle");
  await expect(panel.getByTestId("pki-vehicles-table").locator("th", { hasText: "Trust list" })).toHaveCount(0);
  // With a vehicle open beside it, the table still fits its column.
  await rows.first().click();
  await expect(panel.getByTestId("pki-vehicle-card")).toBeVisible();
  const fit = await panel.locator(".pkir-table-wrap").first().evaluate((el) => el.scrollWidth - el.clientWidth);
  expect(fit).toBeLessThanOrEqual(1);
});

test("A vehicle whose enrolment certificate fails is never 'healthy': its row and its card say the same", async ({ page }) => {
  const tabs = JSON.parse(readFileSync(join(here, "../test/fixtures/pki/pki-tabs.json"), "utf8")) as { vehicles: { vehicles: { node: number; enrolment: string }[] } };
  const v = tabs.vehicles.vehicles.find((x) => x.node === 19)!;
  // A one-hour enrolment certificate whose hour ended an hour ago: the time check fails for a real
  // reason. (A lifetime of zero, the engine's rounding, is a simulator limit: pki-round2.spec.ts.)
  const NOW = 731232071;
  const rules = ["well-formed", "chain-to-root", "root-on-ctl", "signatures", "issuer-permissions", "chain-length", "validity", "region", "crl"];
  const ec = {
    id8: v.enrolment,
    chain: [v.enrolment],
    valid: false,
    time32: NOW,
    t_ns: 66e9,
    tolerated: [],
    trace: rules.map((rule) => ({ rule, status: rule === "validity" ? "fail" : "pass", detail: rule === "validity" ? ["cert 0: not valid at the policy time"] : [] })),
    tree: { issuer: { self: "sha256" }, toBeSigned: { id: { name: "ee" }, validityPeriod: { duration: { hours: 1 }, start: NOW - 7200 } }, type: "explicit", version: 3 },
  };
  await serve(page, studio.run, { [v.enrolment]: ec });
  await open(page, "vehicles");
  const panel = page.getByTestId("pki-panel");
  const row = panel.locator(`[data-testid="pki-vehicle-row"][data-node="19"]`);
  await expect(row).toHaveAttribute("data-state", "healthy");
  await row.click();
  await expect(panel.getByTestId("pki-vehicle-card-state")).toHaveAttribute("data-state", "ec-bad");
  await expect(row).toHaveAttribute("data-state", "ec-bad");
  await expect(panel.getByTestId("pki-glance-ec-says")).toContainText("a receiver reads it as expired");
  await expect(panel.getByTestId("pki-vehicle-card")).not.toContainText("healthy");
});

test("Header and Overview: one header row, the view picker in words, the legend above the diagram, activity in a neutral tone", async ({ page }) => {
  await serve(page, studio.run);
  await open(page, "trust");
  const panel = page.getByTestId("pki-panel");
  await expect(panel.getByTestId("pki-topology")).toBeVisible({ timeout: 30_000 });
  // One row: the tabs, "Seen as" and Close share a line.
  const head = await panel.locator(".pki-head").boundingBox();
  expect(head!.height).toBeLessThan(48);
  await expect(panel.getByTestId("pki-view").locator("option").first()).toHaveText("everything (ground truth)");
  await expect(panel.getByTestId("pki-close")).toContainText("Close");
  await expect(panel.getByTestId("pki-time")).toContainText("Certificate standard: IEEE 1609.2");
  // The legend is read before the diagram.
  const legend = await panel.getByTestId("pki-topo-legend").boundingBox();
  const gov = await panel.getByTestId("pki-governance").boundingBox();
  expect(legend!.y + legend!.height).toBeLessThanOrEqual(gov!.y);
  await expect(panel.getByTestId("pki-topo-legend")).toContainText("busy in the last 2 s");
  // Revoked devices and open cases are the system working: never the error tone.
  for (const id of ["pki-health-revoked", "pki-health-cases", "pki-health-pending"]) {
    await expect(panel.getByTestId(id)).not.toHaveAttribute("data-tone", "bad");
  }
  await expect(panel.getByTestId("pki-health-revoked")).toHaveAttribute("data-tone", /ok|info/);
});

test("Misbehaviour: each case card carries the story's four beats and its numbers", async ({ page }) => {
  await serve(page, studio.run);
  await open(page, "cases");
  const panel = page.getByTestId("pki-panel");
  const cards = panel.getByTestId("pki-case-card");
  await expect(cards.first()).toBeVisible();
  const n = await cards.count();
  for (let k = 0; k < n; k++) {
    const card = cards.nth(k);
    await card.click();
    const beats = await card.getByTestId("pki-case-progress").locator("small").allTextContents();
    expect(beats).toHaveLength(4);
    const story = await panel.getByTestId("pki-case-story").locator("li b").allTextContents();
    expect(beats).toEqual(story);
  }
  // A case on the list says how long that took, and how many vehicles refuse its sender.
  await expect(panel.getByTestId("pki-case-to-list").first()).toContainText("after the first report");
  await expect(panel.getByTestId("pki-case-refusing").first()).toContainText("vehicles refuse it");
});

test("Messages: a message reads as a sentence, and opening it holds its flow while the list grows", async ({ page }) => {
  const live = await serve(page, studio.sequence);
  await open(page, "flows");
  const panel = page.getByTestId("pki-panel");
  await panel.getByTestId("pki-flow-type").selectOption("provisioning");
  const seq = panel.getByTestId("pki-seq");
  await expect(seq).toBeVisible();
  await expect(panel.locator(".pki-flow-title")).toHaveAttribute("title", /\/provisioning\//);
  const title = await panel.locator(".pki-flow-title").getAttribute("title");
  // A message with more than one SPDU opens its card: who sent what to whom, in words.
  await seq.locator('g.pki-msg[data-observed="1"]').first().click();
  const card = panel.getByTestId("pki-msg-card").or(panel.getByTestId("pdu-decoder"));
  await expect(card).toBeVisible();
  live.grow();
  // The list grows; the open message's flow stays, and the newer one is offered, not forced.
  await expect(panel.getByTestId("pki-flow-newer")).toContainText("1 newer flow", { timeout: 15_000 });
  await expect(panel.locator(".pki-flow-title")).toHaveAttribute("title", title!);
  await expect(card).toBeVisible();
  // Every arrow's label stays inside the diagram.
  const width = Number(await seq.getAttribute("width"));
  const over = await seq.locator("text.pki-msg-label").evaluateAll((els, w) => els.filter((e) => {
    const b = (e as SVGGraphicsElement).getBBox();
    return b.x < 0 || b.x + b.width > (w as number);
  }).length, width);
  expect(over).toBe(0);
  // Following the newest drops the hold.
  await panel.getByTestId("pki-flow-newer").click();
  await expect(panel.locator(".pki-flow-title")).toHaveAttribute("title", NEWER);
});

test("A message card says in words who sent what, and the stage names are words", async ({ page }) => {
  await serve(page, studio.sequence);
  await open(page, "flows");
  const panel = page.getByTestId("pki-panel");
  await panel.getByTestId("pki-flow-type").selectOption("provisioning");
  await panel.getByTestId("pki-flow-provider-a/provisioning/2").click();
  const seq = panel.getByTestId("pki-seq");
  // The LA's PLV message carries one SPDU sealed to the ACA: as LA1 it opens as a card first.
  await panel.getByTestId("pki-view").selectOption("provider-a/la1/0");
  await seq.locator("g.pki-msg", { hasText: "EncryptedIndividualPLV" }).first().click();
  const card = panel.getByTestId("pki-msg-card");
  await expect(card.getByTestId("pki-msg-says")).toContainText("sent this to");
  await expect(card.getByTestId("pki-msg-says")).not.toContainText("provider-a/");
  await expect(card).toContainText("readable only by");
  // Stage names in the table are words, not engine identifiers.
  for (const t of await panel.getByTestId("pki-stages").locator("tbody td:first-child").allTextContents()) expect(t).not.toMatch(/[_]/);
});

test("Revocation: the schedule in one sentence per provider, and the table fits at 1280 with a list open", async ({ page }) => {
  await serve(page, studio.run);
  await open(page, "crls");
  const panel = page.getByTestId("pki-panel");
  const rules = panel.getByTestId("pki-crl-schedule");
  await expect(rules.first()).toBeVisible();
  await expect(rules.first()).toContainText(/business day|as soon as|every/);
  await panel.getByTestId("pki-crl-row").first().click();
  await panel.getByTestId("pki-crl-version").first().click();
  await expect(panel.getByTestId("pki-crl-card")).toBeVisible();
  const fit = await panel.locator(".pkir-table-wrap").first().evaluate((el) => el.scrollWidth - el.clientWidth);
  expect(fit).toBeLessThanOrEqual(1);
  // Dates carry the year: one format everywhere.
  const issued = await panel.getByTestId("pki-crl-table").locator(":scope > tbody > tr > td:nth-child(5)").allTextContents();
  expect(issued.length).toBeGreaterThan(0);
  for (const d of issued) expect(d).toMatch(/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}$/);
  // An open row's versions are a table of their own, with their own headers, each column under its header.
  const versions = panel.getByTestId("pki-crl-versions");
  await expect(versions.locator("th").first()).toHaveText("Version");
  for (const d of await versions.locator("tbody td:nth-child(2)").allTextContents()) expect(d).toMatch(/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}$/);
  for (const d of await versions.locator("tbody td:nth-child(5)").allTextContents()) expect(d).toMatch(/^(full list|delta: changes only)$/);
});
