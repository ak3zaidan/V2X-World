/**
 * The metrics dashboard, driven in a real browser against the mock engine: open it (button and
 * the M key), find a metric, expand it, brush a range, read the statistics and a breakdown, export
 * the CSV and the PNG, follow a link to a breakdown, check both themes, close it.
 *
 * The run is left running, as the files sorted after this one expect.
 */

import { expect, test, type Page } from "@playwright/test";
import { readFileSync } from "node:fs";

async function streamingFor(page: Page, seconds: number): Promise<void> {
  await expect(page.getByTestId("connection-state")).toHaveAttribute("data-state", "streaming", { timeout: 60_000 });
  const playing = await page.getByTestId("play").count();
  if (playing > 0) await page.getByTestId("play").click();
  await expect
    .poll(
      async () =>
        page.evaluate(() => {
          const client = window.__vwpStudio?.engine.client;
          return client ? Number((client.poses as unknown as { simTimeNs: bigint }).simTimeNs) / 1e9 : 0;
        }),
      { timeout: 60_000 },
    )
    .toBeGreaterThan(seconds);
}

test("open, find, expand, brush, export and close the metrics dashboard", async ({ page }) => {
  const errors: string[] = [];
  page.on("console", (m) => {
    if (m.type() === "error") errors.push(m.text());
  });
  await page.goto("/");
  await streamingFor(page, 8);

  // --- open with the keyboard; the old strip is gone -------------------------------------------
  await page.locator("body").click({ position: { x: 5, y: 300 } });
  await page.keyboard.press("m");
  await expect(page.getByTestId("metrics-panel")).toBeVisible();
  expect(page.url()).toContain("#metrics");
  await expect(page.getByTestId("plots-strip")).toHaveCount(0);

  // Cards grouped by question; a card with data shows a value, one without says why.
  await expect(page.getByTestId("metrics-group-delivery")).toBeVisible();
  await expect(page.getByTestId("metric-card-pdr")).toHaveAttribute("data-has-data", "true", { timeout: 20_000 });
  await expect(page.getByTestId("metric-value-pdr")).not.toHaveText("—");
  const emptyBoxes = await page.locator('.mcard[data-has-data="false"]').evaluateAll((cards) =>
    cards.filter((c) => (c.querySelector(".mcard-empty")?.textContent ?? "").trim().length < 10).length,
  );
  expect(emptyBoxes, "every card without data says why").toBe(0);

  // --- find a metric by search ---------------------------------------------------------------
  await page.getByTestId("metrics-search").fill("channel busy");
  await expect(page.getByTestId("metric-card-cbr")).toBeVisible();
  await expect(page.getByTestId("metric-card-pdr")).toHaveCount(0);
  await page.getByTestId("metrics-search").fill("");

  // --- pin one: it leads the dashboard and reaches the header ---------------------------------
  await page.getByTestId("metric-pin-cbr").click();
  await expect(page.getByTestId("metrics-group-pinned").getByTestId("metric-card-cbr")).toBeVisible();

  // --- expand ---------------------------------------------------------------------------------
  await page.getByTestId("metric-open-pdr").first().click();
  await expect(page.getByTestId("metric-expanded")).toHaveAttribute("data-metric", "pdr");
  expect(page.url()).toContain("#metrics/pdr");
  await expect(page.getByTestId("metric-chart").locator("canvas")).toHaveCount(1, { timeout: 20_000 });
  await expect(page.getByTestId("metric-source")).toContainText("TR 36.885");
  const statsBefore = await page.getByTestId("metric-stats-pdr").locator("[data-stat=n]").innerText();
  expect(Number(statsBefore.replace(/,/g, ""))).toBeGreaterThan(3);

  // --- brush a range on the strip under the chart ----------------------------------------------
  const brush = page.getByTestId("metric-brush");
  const box = await brush.boundingBox();
  if (!box) throw new Error("no brush");
  await page.mouse.move(box.x + box.width * 0.25, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width * 0.5, box.y + box.height / 2, { steps: 8 });
  await page.mouse.move(box.x + box.width * 0.6, box.y + box.height / 2, { steps: 4 });
  await page.mouse.up();
  await expect.poll(() => page.url()).toMatch(/#metrics\/pdr\?from=[\d.]+&to=[\d.]+/);
  const rangeText = await page.getByTestId("metric-range-text").innerText();
  const [from, to] = rangeText.split(" s of ")[0].split("–").map(Number);
  expect(to - from).toBeGreaterThan(0);
  const statsAfter = Number((await page.getByTestId("metric-stats-pdr").locator("[data-stat=n]").innerText()).replace(/,/g, ""));
  expect(statsAfter, "the statistics cover the brushed range only").toBeLessThan(Number(statsBefore.replace(/,/g, "")));

  // A breakdown over that range: delivery against distance, pooled by the engine.
  await expect(page.getByTestId("breakdown-dist_bin")).toBeVisible();
  await expect(page.getByTestId("breakdown-distance-chart")).toBeVisible({ timeout: 20_000 });
  await expect(page.getByTestId("breakdown-node").locator("tbody tr").first()).toBeVisible();
  // Nodes rank confidently worst first by default; by value, the first row is the lowest value.
  await expect(page.getByTestId("breakdown-node-rank")).toHaveValue("confident");
  await page.getByTestId("breakdown-node-rank").selectOption("value");
  const nodeValues = await page
    .getByTestId("breakdown-node")
    .locator("tbody tr")
    .evaluateAll((rows) => rows.map((r) => Number(r.querySelectorAll("td")[2]?.textContent ?? "NaN")));
  expect(nodeValues.length).toBeGreaterThan(1);
  expect(nodeValues[0]).toBe(Math.min(...nodeValues));

  // --- export ---------------------------------------------------------------------------------
  const [csv] = await Promise.all([page.waitForEvent("download"), page.getByTestId("metric-export-csv").click()]);
  expect(csv.suggestedFilename()).toMatch(/^pdr_.*\.csv$/);
  const csvText = readFileSync(await csv.path(), "utf8");
  const lines = csvText.trim().split(/\r\n/);
  expect(lines[0]).toBe("t_s,pdr (ratio)");
  expect(lines.length).toBeGreaterThan(1);
  for (const line of lines.slice(1)) {
    const t = Number(line.split(",")[0]);
    expect(t).toBeGreaterThanOrEqual(from - 1e-6);
    expect(t).toBeLessThanOrEqual(to + 1e-6);
  }
  const [png] = await Promise.all([page.waitForEvent("download"), page.getByTestId("metric-export-png").click()]);
  expect(png.suggestedFilename()).toMatch(/^pdr_.*\.png$/);
  const bytes = readFileSync(await png.path());
  expect([...bytes.subarray(1, 4)].map((b) => String.fromCharCode(b)).join("")).toBe("PNG");

  // --- a related metric opens over the same range, and leads back -----------------------------
  await page.getByTestId("metric-related-cbr").click();
  await expect(page.getByTestId("metric-expanded")).toHaveAttribute("data-metric", "cbr");
  await expect.poll(() => page.url()).toMatch(/#metrics\/cbr\?from=[\d.]+&to=[\d.]+/);
  await expect(page.getByTestId("metric-range-text")).toContainText(`${rangeText.split(" s of ")[0]} s of`);
  await page.getByTestId("metric-related-pdr").click();
  await expect(page.getByTestId("metric-expanded")).toHaveAttribute("data-metric", "pdr");

  // --- light theme: the same chart, readable --------------------------------------------------
  await page.evaluate(() => document.documentElement.setAttribute("data-theme", "light"));
  const inkLight = await page.getByTestId("metric-stats").evaluate((el) => getComputedStyle(el).color);
  await page.evaluate(() => document.documentElement.removeAttribute("data-theme"));
  const inkDark = await page.getByTestId("metric-stats").evaluate((el) => getComputedStyle(el).color);
  expect(inkLight).not.toBe(inkDark);

  // --- Escape collapses the chart, then closes the dashboard ----------------------------------
  // Even with the focus on the page body, where a click on the chart or on a related metric (whose
  // button unmounts) leaves it: the shell's own Escape would otherwise close the whole dashboard.
  await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("metric-expanded")).toHaveCount(0);
  await expect(page.getByTestId("metrics-panel")).toBeVisible();
  await page.getByTestId("metrics-close").click();
  await expect(page.getByTestId("metrics-panel")).toBeHidden();
  expect(page.url()).not.toContain("#metrics");

  // --- a link opens a breakdown directly ------------------------------------------------------
  await page.goto("/#metrics/pdr/dist_bin?from=1&to=6");
  await expect(page.getByTestId("metric-expanded")).toHaveAttribute("data-metric", "pdr", { timeout: 60_000 });
  await expect(page.getByTestId("breakdown-dist_bin")).toHaveClass(/highlighted/);
  await expect(page.getByTestId("metric-range-text")).toContainText("1–6 s");
  await page.getByTestId("metrics-close").click();
  await expect(page.getByTestId("metrics-panel")).toBeHidden();

  // Opened again, the dashboard is where it was left: the linked chart. Back to the cards, and
  // unpin, so the next file starts from a clean header.
  await page.getByTestId("metrics-button").click();
  await expect(page.getByTestId("metric-expanded")).toHaveAttribute("data-metric", "pdr");
  await page.getByTestId("metric-back").click();
  await page.getByTestId("metric-pin-cbr").first().click();
  await expect(page.getByTestId("metrics-group-pinned")).toHaveCount(0);
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("metrics-panel")).toBeHidden();

  const ignorable = (t: string): boolean => (t.includes("WebGL") && t.includes("deprecat")) || t.includes("GPU stall");
  expect(errors.filter((t) => !ignorable(t))).toEqual([]);
});

// ------------------------------------------------------------------------------------------------
// The credential system at a glance, on a real SCMS run's answers
// ------------------------------------------------------------------------------------------------

/**
 * The real engine's answers at the end of `scenarios/scms-audit-mbpipe.yaml` (two providers, five
 * days of the real calendar): its SCMS catalogue rows, those series, the run-wide groupings and the
 * Backend snapshot (`test/fixtures/metrics/scms-run.json`). The mock engine has no credential
 * system, so the page's socket is routed: the catalogue answer gains the SCMS rows, the backend
 * snapshot and any query of SCMS series alone are answered from the file, and the rest passes on.
 */
type Row = (string | number | null)[];
const scms = JSON.parse(readFileSync(new URL("../test/fixtures/metrics/scms-run.json", import.meta.url), "utf8")) as {
  catalogue: { name: string }[];
  series: Record<string, [number, number | null][]>;
  groups: Record<string, Row[]>;
  backend: { state: { entities: { id: string; state: Record<string, unknown> }[] } };
};
const scmsNames = new Set(scms.catalogue.map((c) => c.name));

/** A count summed over every provider's entity of a role, as the fixture has it. */
function fixtureTotal(role: string, key: string): number {
  return scms.backend.state.entities
    .filter((e) => e.id.split("/").pop() === role)
    .reduce((a, e) => a + (typeof e.state[key] === "number" ? (e.state[key] as number) : 0), 0);
}

function scmsAnswer(p: { metrics: string[]; group_by?: string[] }): unknown {
  const dim = (p.group_by ?? []).find((d) => d !== "t");
  if (dim !== undefined) return { columns: [], rows: scms.groups[`${p.metrics[0]}/${dim}`] ?? [], group_by: p.group_by };
  const grid = [...new Set(p.metrics.flatMap((n) => (scms.series[n] ?? []).map((r) => r[0])))].sort((a, b) => a - b);
  const rows = grid.map((t) => [t, ...p.metrics.map((n) => scms.series[n]?.find((r) => r[0] === t)?.[1] ?? null)]);
  return { columns: [{ name: "t_ns" }, ...p.metrics.map((n) => ({ name: n }))], rows, truncated: false };
}

async function serveScms(page: Page): Promise<void> {
  await page.routeWebSocket(/.*/, (ws) => {
    const server = ws.connectToServer();
    const catalogueIds = new Set<unknown>();
    ws.onMessage((m) => {
      if (typeof m === "string") {
        try {
          const req = JSON.parse(m) as { id?: unknown; method?: string; params?: { entity?: string; metrics?: string[]; group_by?: string[] } };
          if (req.method === "inspect.entity" && req.params?.entity === "backend") {
            ws.send(JSON.stringify({ jsonrpc: "2.0", id: req.id, result: scms.backend }));
            return;
          }
          if (req.method === "metrics.query") {
            const metrics = req.params?.metrics;
            if (!Array.isArray(metrics)) catalogueIds.add(req.id);
            else if (metrics.length > 0 && metrics.every((n) => scmsNames.has(n))) {
              ws.send(JSON.stringify({ jsonrpc: "2.0", id: req.id, result: scmsAnswer({ metrics, group_by: req.params?.group_by }) }));
              return;
            }
          }
        } catch {
          // Not JSON: pass it on.
        }
      }
      server.send(m);
    });
    server.onMessage((m) => {
      if (typeof m === "string" && catalogueIds.size > 0) {
        try {
          const res = JSON.parse(m) as { id?: unknown; result?: { catalogue?: unknown[] } };
          if (catalogueIds.has(res.id) && res.result) {
            catalogueIds.delete(res.id);
            res.result.catalogue = [...(res.result.catalogue ?? []), ...scms.catalogue];
            ws.send(JSON.stringify(res));
            return;
          }
        } catch {
          // Not JSON: pass it on.
        }
      }
      ws.send(m);
    });
  });
}

async function openScms(page: Page): Promise<void> {
  await serveScms(page);
  await page.goto("/");
  await streamingFor(page, 3);
  await page.getByTestId("metrics-button").click();
  await expect(page.getByTestId("metrics-scms")).toBeVisible({ timeout: 30_000 });
}

test.describe("the credential system at a glance", () => {
  test("answers five questions with the run's numbers, adding up both providers", async ({ page }) => {
    await openScms(page);
    const o = page.getByTestId("metrics-scms");
    for (const id of ["certs", "revocation", "misbehaviour", "cost", "backend"]) await expect(o.getByTestId(`scms-section-${id}`)).toBeVisible();
    await expect(o.getByTestId("scms-facts")).toContainText("2 providers");
    // Enrolment adds both providers' devices; the split names each.
    const enrolled = fixtureTotal("ee", "enrolled");
    await expect(o.getByTestId("scms-value-enrolled")).toHaveText(`${enrolled} of ${fixtureTotal("ee", "devices")}`);
    await expect(o.getByTestId("scms-tile-enrolled")).toContainText("provider-b");
    await expect(o.getByTestId("scms-answer-certs")).toContainText(`all ${enrolled} devices are enrolled`);
    // The report funnel starts with what vehicles filed and ends with the MAs' revocations.
    await expect(o.getByTestId("scms-step-filed")).toContainText(fixtureTotal("ee", "reports_filed").toLocaleString("en-US"));
    await expect(o.getByTestId("scms-step-revoked")).toContainText(String(fixtureTotal("ma", "revocations")));
    // Every number says what it means.
    const meanings = await o.locator(".scms-tile-meaning").evaluateAll((els) => els.map((e) => (e.textContent ?? "").trim().length));
    expect(meanings.length).toBeGreaterThan(10);
    expect(Math.min(...meanings)).toBeGreaterThan(15);
  });

  test("judges detection against its published band, pooled over the run", async ({ page }) => {
    await openScms(page);
    const vehicle = scms.groups["det_recall/level"]!.find((r) => r[0] === "vehicle")![1] as number;
    const recall = page.getByTestId("scms-tile-recall");
    await expect(page.getByTestId("scms-value-recall")).toHaveText(String(Math.round(vehicle * 1000) / 10));
    // Below the 90 % band, above 50 %: caution, with the threshold and source in its tooltip.
    await expect(recall).toHaveAttribute("data-level", "caution");
    expect(await recall.getAttribute("title")).toContain("no standard sets a detection rate");
    // A security check's rejections appear by reason.
    await expect(page.getByTestId("scms-bar-psid-not-registered")).toBeVisible();
  });

  test("a number with a chart opens it, by keyboard too; the raw counts are one click away", async ({ page }) => {
    await openScms(page);
    await page.getByTestId("scms-tile-cert_pool_valid").focus();
    await page.keyboard.press("Enter");
    await expect(page.getByTestId("metric-expanded")).toHaveAttribute("data-metric", "cert_pool_valid");
    await page.getByTestId("metric-back").click();
    await page.getByTestId("scms-raw-toggle").click();
    await expect(page.getByTestId("scms-raw")).toContainText("decision_rule");
    // Folded, it stays folded on the next visit.
    await page.getByTestId("scms-toggle").click();
    await expect(page.getByTestId("scms-section-certs")).toHaveCount(0);
    await page.reload();
    await streamingFor(page, 1);
    await expect(page.getByTestId("scms-toggle")).toHaveAttribute("aria-expanded", "false", { timeout: 30_000 });
    await page.getByTestId("scms-toggle").click();
    await expect(page.getByTestId("scms-section-certs")).toBeVisible();
  });

  test("a level reads without colour: each level's mark has its own shape", async ({ page }) => {
    await openScms(page);
    const shapes = await page.evaluate(() =>
      // Pending (not judged yet) is a fourth mark: a hollow ring.
      ["healthy", "caution", "unhealthy", "pending"].map((l) => {
        const el = document.createElement("span");
        el.className = `hdot ${l}`;
        document.body.appendChild(el);
        const s = getComputedStyle(el);
        const shape = `${s.clipPath}|${s.borderRadius}|${s.borderTopWidth}`;
        el.remove();
        return shape;
      }),
    );
    expect(new Set(shapes).size).toBe(4);
  });

  test("the run summary opens with the run in a few sentences", async ({ page }) => {
    await openScms(page);
    await page.getByTestId("metrics-open-summary").click();
    const brief = page.getByTestId("run-summary-brief");
    await expect(brief).toContainText(`All ${fixtureTotal("ee", "devices")} devices were enrolled`, { timeout: 30_000 });
    const sentences = ((await brief.locator("p").innerText()).match(/[.!?](\s|$)/g) ?? []).length;
    expect(sentences).toBeGreaterThanOrEqual(3);
    expect(sentences).toBeLessThanOrEqual(5);
  });

  test("every card says in a line what its metric means", async ({ page }) => {
    await openScms(page);
    await expect(page.getByTestId("metric-meaning-cert_pool_valid")).toContainText("could sign with right now");
    const short = await page.locator(".mcard .mcard-meaning").evaluateAll((els) => els.filter((e) => (e.textContent ?? "").trim().length < 10).length);
    expect(short).toBe(0);
  });

  test("the cards below point at the overview for run totals, instead of saying there is no data", async ({ page }) => {
    await openScms(page);
    // Detection recall has no per-window value, only a run total: its group says where that total is.
    const pooled = page.getByTestId("metrics-pooled-misbehaviour");
    await expect(pooled).toContainText("Credentials at a glance", { timeout: 30_000 });
    await expect(pooled).toContainText("Detection recall");
    await expect(page.getByTestId("metrics-missing-misbehaviour")).not.toContainText("Detection recall");
    // The navigation and the section share one name, and the navigation carries the worst verdict.
    await expect(page.getByTestId("metrics-nav-scms")).toHaveText(await page.locator("#scms-title").innerText());
    const levels = await page.locator("[data-testid^='scms-section-']").evaluateAll((els) => els.map((e) => e.getAttribute("data-level")));
    const worst = ["unhealthy", "caution", "healthy"].find((l) => levels.includes(l));
    expect(worst).toBeDefined();
    await expect(page.getByTestId("metrics-nav-scms")).toHaveAttribute("data-level", worst!);
  });

  test("tiles read at a glance: five a row at most, a number never split from its unit, one decimal for rejections", async ({ page }) => {
    await openScms(page);
    // Once the authorities' counts are in, every question's tiles, row by row (the cost question has seven here).
    await expect(page.getByTestId("scms-tile-enrolled")).toBeVisible({ timeout: 30_000 });
    await expect(page.getByTestId("scms-tile-backend-messages")).toBeVisible();
    await expect.poll(() => page.getByTestId("scms-section-cost").locator(".scms-tile").count()).toBeGreaterThan(5);
    const rows = await page.locator(".scms-tiles").evaluateAll((grids) =>
      grids.flatMap((g) => {
        const tops = [...g.children].map((e) => Math.round(e.getBoundingClientRect().top));
        return [...new Set(tops)].map((t) => tops.filter((x) => x === t).length);
      }),
    );
    expect(rows.length).toBeGreaterThan(3);
    expect(Math.max(...rows)).toBeLessThanOrEqual(5);
    const split = await page.locator(".scms-tile-value").evaluateAll((els) =>
      els.filter((e) => {
        const n = e.querySelector(".scms-num")?.getBoundingClientRect();
        const u = e.querySelector(".scms-unit")?.getBoundingClientRect();
        return n !== undefined && (n.height > 40 || (u !== undefined && (u.height > 24 || Math.abs(u.bottom - n.bottom) > 8)));
      }).length,
    );
    expect(split).toBe(0);
    const shares = await page.locator("[data-testid='scms-bars-cost'] .scms-bar-val").allInnerTexts();
    expect(shares.length).toBeGreaterThan(1);
    for (const s of shares) expect(s).toMatch(/^(\d+\.\d %|\d{2,} %|< 0\.1 %)$/);
    // The roadside-relay PSID drops are named as expected, not as a failed check.
    await expect(page.getByTestId("scms-bar-psid-not-registered")).toContainText("expected");
  });

  test("detection says what is still on its way to the MA; the raw counts open in view", async ({ page }) => {
    await openScms(page);
    const inShuffle = fixtureTotal("ra", "reports_in_shuffle");
    await expect(page.getByTestId("scms-tile-recall")).toContainText(`${inShuffle.toLocaleString("en-US")} reports still wait in the RA's shuffle`);
    await page.getByTestId("scms-raw-toggle").click();
    await expect(page.getByTestId("scms-raw")).toBeInViewport({ timeout: 5000 });
  });

  test("the run summary's credential tile has the authorities' counts", async ({ page }) => {
    await openScms(page);
    await page.getByTestId("metrics-open-summary").click();
    await expect(page.getByTestId("run-summary-cred-revoked")).toContainText(String(fixtureTotal("ma", "revocations")), { timeout: 30_000 });
    await expect(page.getByTestId("run-summary-cred-crl-entries")).toContainText(String(fixtureTotal("crlg", "entries")));
    await expect(page.getByTestId("run-summary-cred-filed")).toContainText(fixtureTotal("ee", "reports_filed").toLocaleString("en-US"));
    await expect(page.getByTestId("run-summary-cred-detection")).toBeVisible();
  });
});
