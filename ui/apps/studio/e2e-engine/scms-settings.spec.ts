/**
 * Every SCMS setting is a menu, a number with its unit, a toggle, a map or a pair: never a bare text
 * box or a JSON box (docs/design/scms-real/08-settings-ux-and-gaps.md §10, the owner's rule "nobody
 * should have to know what to type"). Against the real engine's published schema.
 *
 * The window opens Certificates & SCMS with Advanced shown, checks the eight sub-headings the engine
 * publishes (`x-subgroup`, 08 §3), and reads `data-kind` off every row whose pointer is under
 * `/security`, `/actors/backend` or `/detection/ma`. The one JSON row allowed is
 * `/security/protocol/params`, the legacy spelling, which must say so.
 *
 * A built-in twin proves the check can fail: the same assertion runs with the engine's schema
 * answer rewritten in flight so the butterfly row carries no `x-options` (and so, as the engine
 * itself would publish it, `kind: "string"`). That twin is `test.fail()`: if the check stops being
 * able to fail, the twin passes and the suite goes red.
 */

import { expect, test, type Page } from "@playwright/test";

import { EngineProcess, open, openSettings, writeScenarios } from "./support.js";

const scenarios = writeScenarios();
const engine = new EngineProcess(scenarios.a);

test.describe.configure({ mode: "serial" });

test.beforeAll(async () => {
  await engine.start();
});
test.afterAll(async () => {
  await engine.stop();
});

const SUBGROUPS = [
  "Deployment",
  "Providers",
  "Certificate lifecycle",
  "Revocation",
  "Misbehaviour authority",
  "Receiver checks",
  "Cryptography",
  "Failures & servers",
];
const BUTTERFLY = "security.scms.deployment.options.butterfly";

/** Opens Certificates & SCMS with every row shown, and returns each SCMS row's pointer and kind. */
async function scmsRows(page: Page): Promise<{ pointer: string; kind: string; text: string }[]> {
  await open(page);
  await openSettings(page);
  await page.locator('[data-testid="settings-tree-group"][data-group="Certificates & SCMS"]').click();
  const section = page.locator('[data-testid="settings-section"]').first();
  const adv = section.getByTestId("settings-advanced-toggle");
  if ((await adv.count()) > 0 && (await adv.getAttribute("aria-expanded")) !== "true") await adv.click();
  await expect(section.locator('[data-testid="setting"]').first()).toBeVisible();
  const headings = await section.getByTestId("settings-subheading").allTextContents();
  for (const g of SUBGROUPS) expect(headings, `the SCMS page has the "${g}" sub-heading`).toContain(g);
  return section.locator('[data-testid="setting"]').evaluateAll((els) =>
    els.map((e) => ({
      pointer: e.getAttribute("data-pointer") ?? "",
      kind: e.getAttribute("data-kind") ?? "",
      text: (e.textContent ?? "").slice(0, 400),
    })),
  );
}

/** The §10 check: no SCMS row is a bare text box or a JSON box, except the legacy params. */
function check(rows: { pointer: string; kind: string; text: string }[]): void {
  const scms = rows.filter((r) => ["/security", "/actors/backend", "/detection/ma"].some((p) => r.pointer.startsWith(p)));
  expect(scms.length, "the SCMS page lists its settings").toBeGreaterThan(60);
  const legacy = scms.find((r) => r.pointer === "/security/protocol/params");
  if (legacy) expect(legacy.text, "the params box says it is the legacy spelling").toMatch(/legacy spelling/i);
  const bare = scms.filter((r) => r.pointer !== "/security/protocol/params" && (r.kind === "text" || r.kind === "json" || r.kind === ""));
  expect(bare.map((r) => `${r.pointer} (${r.kind || "no kind"})`), "SCMS settings without a menu").toEqual([]);
}

test("every SCMS setting on the Certificates & SCMS page is a menu, never a text or JSON box", async ({ page }) => {
  check(await scmsRows(page));
});

test("the check fails when the engine stops publishing a menu (the butterfly row loses its x-options)", async ({ page }) => {
  test.fail();
  await page.routeWebSocket(/\/vwp\/v1/, (ws) => {
    const server = ws.connectToServer();
    server.onMessage((message) => {
      if (typeof message !== "string" || !message.includes(BUTTERFLY)) {
        ws.send(message);
        return;
      }
      const strip = (node: unknown): void => {
        if (Array.isArray(node)) {
          node.forEach(strip);
          return;
        }
        if (node === null || typeof node !== "object") return;
        const o = node as Record<string, unknown>;
        if (o["x-path"] === BUTTERFLY) {
          delete o["x-options"];
          delete o["x-options-open"];
          if (o.kind === "enum") o.kind = "string";
        }
        Object.values(o).forEach(strip);
      };
      const doc = JSON.parse(message) as unknown;
      strip(doc);
      ws.send(JSON.stringify(doc));
    });
  });
  check(await scmsRows(page));
});
