/**
 * The pure half of "Compare with previous run": which rows the table shows, how a change is
 * written, and what the agent is handed. The comparison itself (settings diff, pooled values,
 * direction, health, noise test, summary sentences) is computed once, by the engine
 * (`crates/v2xw-server/src/compare.rs`, `runs.compare`), so the Studio, the agent and
 * `v2xw compare` print the same numbers. No DOM here, so `test/run-compare.test.ts` pins it.
 */

import type { RunMetricRow, RunsCompareResult, RunHeader } from "@vwp/protocol";

import { labelOf, formatWithUnit } from "./model.js";

/** A metric's plain label: `latency.p95` → "Latency (p95)". */
export function rowLabel(name: string): string {
  const m = /^(.*)\.(p50|p95|p99)$/.exec(name);
  if (m) return `${labelOf(m[1]!)} (${m[2]})`;
  return labelOf(name);
}

/** The rows shown: meaningful ones only unless `all`, filtered by a search string. */
export function visibleRows(rows: readonly RunMetricRow[], all: boolean, query: string): RunMetricRow[] {
  const q = query.trim().toLowerCase();
  return rows.filter(
    (r) => (all || r.meaningful) && (q === "" || r.name.toLowerCase().includes(q) || rowLabel(r.name).toLowerCase().includes(q)),
  );
}

/** "+0.051 ratio" / "−3.2 ms": a signed change with its unit. */
export function deltaText(r: RunMetricRow): string {
  if (r.delta === null) return "—";
  const sign = r.delta > 0 ? "+" : r.delta < 0 ? "−" : "±";
  return `${sign}${formatWithUnit(Math.abs(r.delta), r.unit)}`;
}

/** "+12.0 %"; empty when the baseline is zero or missing. */
export function pctText(r: RunMetricRow): string {
  if (r.pct === null || !Number.isFinite(r.pct)) return "";
  const sign = r.pct > 0 ? "+" : r.pct < 0 ? "−" : "±";
  return `${sign}${Math.abs(r.pct).toFixed(Math.abs(r.pct) >= 10 ? 0 : 1)} %`;
}

/** The CSS class a row's change is coloured with. */
export function verdictClass(r: RunMetricRow): "better" | "worse" | "neutral" {
  if (r.verdict === "better") return "better";
  if (r.verdict === "worse") return "worse";
  return "neutral";
}

/** "Previous run · seed 1 · 300 s". */
export function sideText(h: RunHeader): string {
  const parts = [h.label, `seed ${h.seed}`];
  if (h.city) parts.push(h.city);
  parts.push(h.finished ? `${Math.round(h.t_end_s)} s` : `${Math.round(h.t_reached_s)} of ${Math.round(h.t_end_s)} s`);
  if (h.replications > 1) parts.push(`${h.replications} replications`);
  return parts.join(" · ");
}

/** What the agent is asked: both runs, the settings that differ and the meaningful changes. */
export function agentPrompt(c: RunsCompareResult): string {
  const lines: string[] = [];
  lines.push(
    `Compare these two runs and explain the differences. Run A (baseline): ${c.a.run_id} (${sideText(c.a)}). Run B: ${c.b.run_id} (${sideText(c.b)}).`,
  );
  lines.push(`The engine's comparison is runs.compare {a: "${c.a.run_id}", b: "${c.b.run_id}"}. ${c.noise_note}`);
  lines.push(c.settings.length === 0 ? "No setting differs." : `Settings changed: ${c.settings.map((s) => `${s.label} (${s.path}) ${s.text}`).join("; ")}.`);
  const top = c.rows.filter((r) => r.meaningful).slice(0, 12);
  if (top.length > 0) {
    lines.push("Meaningful metric changes (A → B):");
    for (const r of top) lines.push(`- ${r.name}: ${formatWithUnit(r.a, r.unit)} → ${formatWithUnit(r.b, r.unit)} (${pctText(r) || deltaText(r)}, ${r.verdict})`);
  } else {
    lines.push("No metric changed meaningfully.");
  }
  lines.push("Is each change the direction and size the changed settings should cause in a real V2X deployment? Say which differences are expected, which are surprising, and what to check.");
  return lines.join("\n");
}

/** A file name for an export: `compare-<a>-vs-<b>.<ext>`. */
export function exportName(c: RunsCompareResult, ext: string): string {
  const short = (id: string): string => id.replace(/[^A-Za-z0-9-]/g, "").slice(0, 8) || "run";
  return `compare-${short(c.a.run_id)}-vs-${short(c.b.run_id)}.${ext}`;
}

/** Counts for the table's header: meaningful, better, worse. */
export function tally(rows: readonly RunMetricRow[]): { meaningful: number; better: number; worse: number; total: number } {
  let meaningful = 0;
  let better = 0;
  let worse = 0;
  let total = 0;
  for (const r of rows) {
    if (r.delta !== null) total++;
    if (!r.meaningful) continue;
    meaningful++;
    if (r.verdict === "better") better++;
    if (r.verdict === "worse") worse++;
  }
  return { meaningful, better, worse, total };
}
