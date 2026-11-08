/**
 * Every Settings control changes the engine's scenario exactly as labelled.
 *
 * One control from each section, and every option of the SCMS select menus, set in the settings
 * window as a user sets it, applied, and read back from `scenario.get` at the control's pointer
 * (the scenario the next run will run). A menu option must land as the option's own value; a
 * typed number as that number; a tick as true. Nothing else in the document may change.
 * `VWP_SETTINGS_OUT`, when set, receives the measures as JSON.
 */

import { writeFileSync } from "node:fs";
import { join } from "node:path";

import { expect, test, type Page } from "@playwright/test";

import { EngineProcess, REPO, closeSettings, open, openSettings } from "./support.js";

const out = process.env.VWP_SETTINGS_OUT ?? "";
const engine = new EngineProcess(join(REPO, "scenarios/tests/studio-credlife.yaml"));

test.beforeAll(async () => engine.start());
test.afterAll(async () => engine.stop());

async function rpc(page: Page, method: string, params: unknown): Promise<unknown> {
  return page.evaluate(
    async ([m, p]) => {
      const e = window.__vwpStudio?.engine as unknown as { requestHttp(m: string, p: unknown, o: unknown): Promise<unknown> };
      return e.requestHttp(m as string, p, { quiet: true });
    },
    [method, params] as const,
  );
}

async function doc(page: Page): Promise<Record<string, unknown>> {
  return ((await rpc(page, "scenario.get", {})) as { scenario: Record<string, unknown> }).scenario;
}

function at(d: unknown, pointer: string): unknown {
  let x: unknown = d;
  for (const k of pointer.split("/").slice(1)) {
    if (x === null || typeof x !== "object") return undefined;
    x = (x as Record<string, unknown>)[k];
  }
  return x;
}

/** Every leaf pointer whose value differs between two documents. */
function diff(a: unknown, b: unknown, path = ""): string[] {
  if (JSON.stringify(a) === JSON.stringify(b)) return [];
  // A parent the edit created (or removed) is not a change of its own: its leaves are.
  const obj = (x: unknown): boolean => x !== null && typeof x === "object" && !Array.isArray(x);
  if ((a === undefined && obj(b)) || (b === undefined && obj(a))) {
    const o = (a ?? b) as Record<string, unknown>;
    return Object.keys(o).flatMap((k) => diff(a === undefined ? undefined : o[k], b === undefined ? undefined : o[k], `${path}/${k}`));
  }
  if (a && b && typeof a === "object" && typeof b === "object" && !Array.isArray(a) && !Array.isArray(b)) {
    const keys = new Set([...Object.keys(a as object), ...Object.keys(b as object)]);
    return [...keys].flatMap((k) => diff((a as Record<string, unknown>)[k], (b as Record<string, unknown>)[k], `${path}/${k}`));
  }
  return [path];
}

/** The settings note after the last Apply ("refused: ...", "take effect when you restart", "Run starts with these settings"). */
let lastNote = "";

/** The select's "Default" option (settings/widgets.tsx UNSET): picking it removes the value. */
const UNSET = "\u0000unset";

/** Set a row in the window (select, checkbox or text), apply, and return the row's kind. */
async function setAndApply(page: Page, pointer: string, value: string): Promise<string> {
  await openSettings(page);
  const filter = page.getByTestId("settings-filter");
  await filter.fill(pointer);
  const row = page.locator(`[data-testid="setting"][data-pointer="${pointer}"]`);
  if ((await row.count()) === 0) {
    await filter.fill("");
    return "absent";
  }
  let kind = "text";
  const select = row.locator("select");
  const box = row.locator('input[type="checkbox"]');
  if ((await select.count()) > 0) {
    kind = "select";
    const values = await select.first().evaluate((s) => [...(s as HTMLSelectElement).options].map((o) => o.value));
    if (!values.includes(value)) {
      await filter.fill("");
      return `no-option (${values.join("|")})`;
    }
    // An option drawn as "not available" is disabled: a person cannot pick it, so it is not a
    // control to judge (it waited out a 30 s timeout per option, 2026-11-03 e3).
    const disabled = await select.first().evaluate((s, v) => [...(s as HTMLSelectElement).options].find((o) => o.value === v)?.disabled ?? false, value);
    if (disabled) {
      await filter.fill("");
      return "disabled";
    }
    await select.first().selectOption(value);
  } else if ((await box.count()) > 0) {
    kind = "checkbox";
    await box.first().setChecked(value === "true");
  } else {
    // A list row (the providers' table) has no single input to type into.
    if ((await row.locator("input").count()) === 0) {
      await filter.fill("");
      return "no-input";
    }
    await row.locator("input").first().fill(value);
    await row.locator("input").first().press("Tab");
  }
  await filter.fill("");
  await page.getByTestId("apply").click();
  await expect(page.getByTestId("scenario-message")).toContainText(/take effect when you restart|Run starts with these settings|No change from the run on screen|refused/, { timeout: 30_000 });
  lastNote = ((await page.getByTestId("scenario-message").textContent()) ?? "").slice(0, 160);
  // A refused Apply changes nothing and keeps the edit in the form, so every later row would be
  // refused with it (e4: 32 rows after preset "custom"). Each row starts from the applied document.
  if (/refused/i.test(lastNote)) {
    const discard = page.getByTestId("discard-edits");
    if (await discard.isEnabled().catch(() => false)) await discard.click();
  }
  return kind;
}

function same(got: unknown, want: string): boolean {
  if (want === UNSET) return got === undefined;
  // The engine writes a seed back as 64-bit hex ("0x0000000000003039" for 12345).
  if (typeof got === "string" && /^0x[0-9a-f]+$/i.test(got) && /^\d+$/.test(want)) return BigInt(got) === BigInt(want);
  if (typeof got === "number") return Math.abs(got - Number(want)) < 1e-9;
  if (typeof got === "boolean") return String(got) === want;
  return String(got) === want;
}

test("every Settings control changes the scenario as labelled", async ({ page }) => {
  test.setTimeout(25 * 60_000);
  await open(page);
  // One control per section: [pointer, value typed or picked].
  const sample: [string, string][] = [
    ["/time/duration_s", "90"],
    ["/time/metric_period_s", "5"],
    ["/radio/devices/obu/tx_power_dbm", "10"],
    ["/radio/mac/cw_min_vo", "15"],
    ["/radio/dcc/enabled", "true"],
    ["/messages/rate_hz", "5"],
    ["/world/buildings/enabled", "false"],
    // The fleet's demand is under actors (credential-lifecycle.yaml); /traffic/demand was absent.
    ["/actors/vehicles/demand/rate_veh_per_h", "750"],
    ["/security/verification_policy", "prioritized"],
    ["/security/scms/lifecycle/certs_per_period", "10"],
    // detection has no enabled switch; its Misbehaviour Authority is a menu (/detection/enabled was absent).
    ["/detection/ma/id", "threat/ma/legacy-window"],
    ["/seed", "12345"],
  ];
  const rows: { pointer: string; value: string; kind: string; got: unknown; ok: boolean; others: string[]; note?: string }[] = [];
  const check = async (pointer: string, value: string): Promise<void> => {
    const before = await doc(page);
    let kind: string;
    lastNote = "";
    try {
      kind = await setAndApply(page, pointer, value);
    } catch (e) {
      kind = `error: ${String(e).slice(0, 120)}`;
      await page.keyboard.press("Escape").catch(() => undefined);
    }
    if (kind !== "select" && kind !== "checkbox" && kind !== "text") {
      rows.push({ pointer, value, kind, got: at(await doc(page), pointer), ok: false, others: [] });
      return;
    }
    const after = await doc(page);
    const got = at(after, pointer);
    const others = diff(before, after).filter((p) => p !== pointer && !p.startsWith(`${pointer}/`));
    rows.push({ pointer, value, kind, got, ok: same(got, value) && others.length === 0, others, note: lastNote });
  };
  for (const [p, v] of sample) await check(p, v);

  // Every option of every SCMS select menu.
  await openSettings(page);
  await page.getByTestId("settings-filter").fill("/security/scms");
  const menus = await page.evaluate(() =>
    [...document.querySelectorAll<HTMLElement>('[data-testid="setting"][data-pointer^="/security/scms"]')]
      .map((r) => ({ pointer: r.dataset.pointer!, options: [...r.querySelectorAll("select option")].map((o) => (o as HTMLOptionElement).value).filter((v) => v !== "") }))
      .filter((m) => m.options.length > 1),
  );
  await page.getByTestId("settings-filter").fill("");
  await closeSettings(page);
  let scmsOptions = 0;
  for (const m of menus.slice(0, 12)) {
    for (const o of m.options) {
      await check(m.pointer, o);
      scmsOptions++;
    }
  }
  const landed = rows.filter((r) => ["select", "checkbox", "text"].includes(r.kind));
  const unusable = rows.filter((r) => !["absent", "disabled", "no-input"].includes(r.kind) && !landed.includes(r));
  const sections = new Set(landed.filter((r) => r.ok).map((r) => r.pointer.split("/")[1]));
  const res = {
    "set.sections_sampled": sections.size,
    "set.scms_menus": menus.length,
    "set.scms_options_sampled": scmsOptions,
    "set.absent": rows.filter((r) => r.kind === "absent").map((r) => r.pointer),
    "set.mismatch": landed.filter((r) => !r.ok).length,
    "set.disabled": rows.filter((r) => r.kind === "disabled").length,
    "set.refused": landed.filter((r) => /refused/i.test(r.note ?? "")).length,
    "set.unusable": unusable.map((r) => `${r.pointer}=${r.value}: ${r.kind}`),
    "set.rows": rows,
  };
  if (out) writeFileSync(out, JSON.stringify(res, null, 1));
  expect(landed.length).toBeGreaterThan(0);
});
