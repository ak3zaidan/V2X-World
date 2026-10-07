/**
 * The signature-scheme comparison's pure half: the document `v2xw compare-signatures` writes
 * (`comparison.json`, schema `v2xw/signature-comparison/1`, crates/v2xw-cli/src/sigcompare.rs), the
 * client for the one-click endpoint (`/agent/compare-signatures`), and the CSV and Markdown exports.
 *
 * The engine computes every number, difference and call-out; this file only formats them, so the
 * table on screen, the CSV and the Markdown say the same thing as the CLI's files.
 */

export const COMPARISON_SCHEMA = "v2xw/signature-comparison/1";

/** The schemes the registry ships (`v2xw_sec::scheme::SIGNATURE_IDS`), in its order. */
export const SIGNATURE_SCHEMES: readonly { id: string; label: string; mode: string }[] = [
  { id: "ecdsa-p256", label: "ECDSA P-256", mode: "classical" },
  { id: "ecdsa-brainpoolp256r1", label: "ECDSA brainpoolP256r1", mode: "classical" },
  { id: "ecdsa-p384", label: "ECDSA P-384", mode: "classical" },
  { id: "ml-dsa-44", label: "ML-DSA-44", mode: "post-quantum" },
  { id: "ml-dsa-65", label: "ML-DSA-65", mode: "post-quantum" },
  { id: "ml-dsa-87", label: "ML-DSA-87", mode: "post-quantum" },
  { id: "falcon-512", label: "Falcon-512", mode: "post-quantum" },
  { id: "falcon-1024", label: "Falcon-1024", mode: "post-quantum" },
  { id: "slh-dsa-sha2-128f", label: "SLH-DSA-SHA2-128f", mode: "post-quantum" },
  { id: "slh-dsa-sha2-128s", label: "SLH-DSA-SHA2-128s", mode: "post-quantum" },
  { id: "hybrid-mldsa44-ecdsa-p256", label: "ML-DSA-44 + P-256", mode: "hybrid" },
  { id: "hybrid-mldsa65-ecdsa-p256", label: "ML-DSA-65 + P-256", mode: "hybrid" },
  { id: "hybrid-falcon512-ecdsa-p256", label: "Falcon-512 + P-256", mode: "hybrid" },
];

export const DEFAULT_SELECTION: readonly string[] = ["ecdsa-p256", "ml-dsa-44", "falcon-512", "hybrid-mldsa44-ecdsa-p256"];

export interface SchemeColumn {
  readonly id: string;
  readonly label: string;
  readonly mode: string;
  readonly sig_octets: number;
  readonly key_octets: number;
  readonly cert_extra_octets: number;
  readonly quantum_level: number;
  readonly sign_engine: string;
  readonly verify_engine: string;
  readonly run_id: string;
}

export interface ComparisonRow {
  readonly key: string;
  readonly label: string;
  readonly group: string;
  readonly unit: string;
  readonly better: "lower" | "higher" | null;
  readonly values: readonly (number | null)[];
  readonly delta: readonly (number | null)[];
  readonly rel: readonly (number | null)[];
  readonly level: readonly (string | null)[];
  readonly highlight: readonly boolean[];
}

export interface Comparison {
  readonly schema: string;
  readonly scenario: string;
  readonly seed_hex: string;
  readonly duration_s: number;
  readonly baseline: string;
  readonly schemes: readonly SchemeColumn[];
  readonly rows: readonly ComparisonRow[];
  readonly caveats: readonly string[];
  readonly files?: readonly string[];
}

export interface CompareJob {
  readonly running: boolean;
  readonly schemes: readonly string[];
  readonly done: number;
  readonly out: string | null;
  readonly error: string | null;
  readonly result: Comparison | null;
}

/** Reads a `comparison.json`, refusing anything else. */
export function parseComparison(text: string): Comparison {
  const doc = JSON.parse(text) as Partial<Comparison>;
  if (doc.schema !== COMPARISON_SCHEMA || !Array.isArray(doc.schemes) || !Array.isArray(doc.rows)) {
    throw new Error(`not a signature comparison (expected schema ${COMPARISON_SCHEMA})`);
  }
  return doc as Comparison;
}

/** The rows a chart shows: the quantities a reader of a scheme comparison asks about first. */
export const CHART_KEYS: readonly string[] = [
  "sig_signature_bytes",
  "sig_security_airtime_share",
  "sig_sign_latency.p95",
  "sig_verify_latency.p95",
  "sig_neighbour_capacity",
  "sig_e2e_added.p95",
  "cbr",
  "sig_reassembly_loss",
];

/** A value for a table cell: three significant places below ten, fewer above. */
export function formatValue(v: number | null): string {
  if (v === null || !Number.isFinite(v)) return "—";
  const a = Math.abs(v);
  if (a >= 1000) return v.toFixed(0);
  if (a >= 10) return v.toFixed(1);
  return v.toFixed(3);
}

/** A relative difference as a signed percentage. */
export function formatRel(r: number | null): string {
  if (r === null || !Number.isFinite(r)) return "";
  const p = r * 100;
  return `${p >= 0 ? "+" : ""}${p.toFixed(0)} %`;
}

/** Whether a difference is better, worse or neither, by the row's direction. */
export function verdict(row: ComparisonRow, i: number): "better" | "worse" | null {
  const d = row.delta[i];
  if (d === null || d === undefined || d === 0 || row.better === null) return null;
  return (row.better === "lower") === d < 0 ? "better" : "worse";
}

/** The long-form CSV, one line per (row, scheme) — the same columns the CLI writes. */
export function comparisonCsv(c: Comparison): string {
  const q = (s: string): string => `"${s.replace(/"/g, '""')}"`;
  const n = (v: number | null | undefined): string => (v === null || v === undefined ? "" : String(v));
  const lines = ["group,key,label,unit,scheme,mode,value,delta,rel,level,highlight"];
  for (const r of c.rows) {
    c.schemes.forEach((s, i) => {
      lines.push(
        [q(r.group), r.key, q(r.label), q(r.unit), s.id, s.mode, n(r.values[i]), n(r.delta[i]), n(r.rel[i]), r.level[i] ?? "", String(r.highlight[i] ?? false)].join(","),
      );
    });
  }
  return `${lines.join("\n")}\n`;
}

/** The Markdown table, differences in brackets and called-out ones in bold. */
export function comparisonMarkdown(c: Comparison): string {
  const out: string[] = [
    `# Signature schemes compared: ${c.scenario}`,
    "",
    `Same scenario, seed ${c.seed_hex} and ${c.duration_s} s of simulated time under every scheme; differences against **${c.baseline}** in brackets, called-out ones in bold.`,
    "",
    "| Scheme | Mode | Signature B | Key B | Certificate +B | Signs on | Verifies on |",
    "|---|---|---|---|---|---|---|",
    ...c.schemes.map((s) => `| ${s.id} | ${s.mode} | ${s.sig_octets} | ${s.key_octets} | ${s.cert_extra_octets} | ${s.sign_engine} | ${s.verify_engine} |`),
  ];
  let group = "";
  for (const r of c.rows) {
    if (r.group !== group) {
      group = r.group;
      out.push("", `## ${group}`, "", `| Quantity | Unit | ${c.schemes.map((s) => s.id).join(" | ")} |`, `|---|---|${c.schemes.map(() => "---|").join("")}`);
    }
    const cells = c.schemes.map((s, i) => {
      let cell = formatValue(r.values[i] ?? null);
      const rel = r.rel[i] ?? null;
      if (rel !== null && s.id !== c.baseline) cell += ` (${formatRel(rel)})`;
      return r.highlight[i] ? `**${cell}**` : cell;
    });
    out.push(`| ${r.label} | ${r.unit} | ${cells.join(" | ")} |`);
  }
  out.push("", "## Caveats", "", ...c.caveats.map((x) => `- ${x}`), "");
  return out.join("\n");
}
