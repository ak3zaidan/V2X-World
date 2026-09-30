/**
 * The mock engine's `metrics.query` (§6.12): a catalogue, a history of every metric bin it sent,
 * time-binned series over any range, and grouped breakdowns — in the shapes the real engine answers
 * (`crates/v2xw-server/src/introspect.rs`), so the Studio's metrics dashboard can be driven end to
 * end against the fixture.
 *
 * The values are the fixture's synthetic ones and say so in their definitions. The breakdowns are
 * derived deterministically from the history (a delivery ratio that falls with distance, a per-node
 * figure spread around the run's mean), which is enough to exercise every chart without claiming to
 * be a measurement.
 */

export interface MockMetricDef {
  readonly name: string;
  readonly unit: string;
  readonly dims: readonly string[];
  readonly agg: string;
  readonly visibility: "NODE" | "GT";
  readonly definition: string;
  readonly source: string;
  readonly notAccounted: readonly string[];
}

const FIXTURE = "The mock server's synthetic value, for exercising the Studio; not a measurement.";

export const MOCK_CATALOGUE: readonly MockMetricDef[] = [
  {
    name: "pdr",
    unit: "ratio",
    dims: ["t", "dist_bin", "node"],
    agg: "ratio",
    visibility: "NODE",
    definition: `Packet delivery ratio: receivers in range that decoded the frame, over receivers in range. ${FIXTURE}`,
    source: "3GPP TR 36.885 §A.2 (PRR)",
    notAccounted: ["everything: the fixture draws it"],
  },
  {
    name: "cbr",
    unit: "ratio",
    dims: ["t", "node"],
    agg: "mean",
    visibility: "NODE",
    definition: `Channel busy ratio: the share of a window the channel was sensed busy. ${FIXTURE}`,
    source: "SAE J2945/1 §6.3.8",
    notAccounted: ["everything: the fixture draws it"],
  },
  {
    name: "msgs_per_s",
    unit: "1/s",
    dims: ["t"],
    agg: "rate",
    visibility: "NODE",
    definition: `Messages sent per second by the equipped fleet. ${FIXTURE}`,
    source: "",
    notAccounted: ["everything: the fixture draws it"],
  },
  {
    name: "verify_wait_p95_ms",
    unit: "ms",
    dims: ["t", "msg_type"],
    agg: "p95",
    visibility: "NODE",
    definition: `The 95th percentile of the wait for signature verification. ${FIXTURE}`,
    source: "",
    notAccounted: ["everything: the fixture draws it"],
  },
  {
    name: "nbr_verified_mean",
    unit: "count",
    dims: ["t"],
    agg: "mean",
    visibility: "NODE",
    definition: `Verified neighbours per equipped vehicle. ${FIXTURE}`,
    source: "",
    notAccounted: ["everything: the fixture draws it"],
  },
  {
    name: "crl_bytes_p95",
    unit: "B",
    dims: ["t"],
    agg: "p95",
    visibility: "NODE",
    definition: `The 95th percentile of the certificate revocation list's size. ${FIXTURE}`,
    source: "IEEE 1609.2.1 §6.3.5",
    notAccounted: ["everything: the fixture draws it"],
  },
  {
    name: "mean_speed",
    unit: "m/s",
    dims: ["t"],
    agg: "mean",
    visibility: "GT",
    definition: `Mean speed of the vehicles on the network. ${FIXTURE}`,
    source: "",
    notAccounted: ["everything: the fixture draws it"],
  },
  {
    name: "ttc_min",
    unit: "s",
    dims: ["t"],
    agg: "min",
    visibility: "GT",
    definition: `The smallest time to collision between two road users in the window. ${FIXTURE}`,
    source: "Hayward (1972)",
    notAccounted: ["everything: the fixture draws it"],
  },
  {
    name: "det_precision",
    unit: "ratio",
    dims: ["t"],
    agg: "ratio",
    visibility: "GT",
    definition: `Of the vehicles the detectors flagged, the share that were attackers. ${FIXTURE}`,
    source: "",
    notAccounted: ["everything: the fixture draws it"],
  },
];

/** The catalogue as `metrics.query` with no metric list answers it. */
export function catalogueAnswer(): unknown[] {
  return MOCK_CATALOGUE.map((m) => ({
    name: m.name,
    unit: m.unit,
    dims: m.dims,
    agg: m.agg,
    visibility: m.visibility,
    definition_md: m.definition,
    not_accounted: m.notAccounted,
    source: m.source === "" ? null : { ref: m.source },
    base: m.name,
  }));
}

/** A small deterministic hash in [0, 1), for per-node and per-bin spreads. */
function unit(key: string): number {
  let h = 2166136261;
  for (let i = 0; i < key.length; i++) {
    h ^= key.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return ((h >>> 0) % 100_000) / 100_000;
}

/** Every metric bin the mock sent, by name, in ascending time. */
export class MockMetricHistory {
  readonly #rows = new Map<string, [number, number][]>();

  record(tNs: number, samples: readonly { readonly name: string; readonly value: number }[]): void {
    for (const s of samples) {
      const list = this.#rows.get(s.name) ?? [];
      // A step replayed after a backward seek rewrites what follows it.
      while (list.length > 0 && list[list.length - 1][0] >= tNs) list.pop();
      list.push([tNs, s.value]);
      this.#rows.set(s.name, list);
    }
  }

  /** Forget everything after `tNs` (a seek backwards). */
  truncateAfter(tNs: number): void {
    for (const list of this.#rows.values()) while (list.length > 0 && list[list.length - 1][0] > tNs) list.pop();
  }

  clear(): void {
    this.#rows.clear();
  }

  /** `[t_ns, v1, v2, …]` rows, each the mean of the samples in `[edge, edge + bin)`; `null` where none. */
  series(names: readonly string[], fromNs: number, toNs: number, binNs: number, limit: number): (number | null)[][] {
    const bin = Math.max(1, binNs);
    const out: (number | null)[][] = [];
    for (let edge = fromNs - (fromNs % bin); edge <= toNs && out.length < limit; edge += bin) {
      const row: (number | null)[] = [edge];
      for (const n of names) {
        const inside = (this.#rows.get(n) ?? []).filter(([t]) => t >= edge && t < edge + bin);
        row.push(inside.length === 0 ? null : inside.reduce((a, [, v]) => a + v, 0) / inside.length);
      }
      out.push(row);
    }
    return out;
  }

  /** The mean of `name` over `[fromNs, toNs]`, with its sample count. */
  #mean(name: string, fromNs: number, toNs: number): { mean: number; n: number } | null {
    const inside = (this.#rows.get(name) ?? []).filter(([t]) => t >= fromNs && t <= toNs);
    if (inside.length === 0) return null;
    return { mean: inside.reduce((a, [, v]) => a + v, 0) / inside.length, n: inside.length };
  }

  /**
   * Grouped rows `[key, value, lo, hi, n]` for one metric by one dimension over a range: the
   * fixture's deterministic spread around the range's mean. No rows for a dimension the metric
   * does not declare, or a range with no samples.
   */
  groups(name: string, dim: string, fromNs: number, toNs: number, nodes: readonly number[]): (string | number | null)[][] {
    const def = MOCK_CATALOGUE.find((m) => m.name === name);
    const base = this.#mean(name, fromNs, toNs);
    if (!def || !def.dims.includes(dim) || base === null) return [];
    const ratio = def.unit === "ratio";
    const clamp = (v: number): number => (ratio ? Math.min(1, Math.max(0, v)) : Math.max(0, v));
    const wilson = (p: number, n: number): [number, number] => {
      const z = 1.96;
      const d = 1 + (z * z) / n;
      const c = (p + (z * z) / (2 * n)) / d;
      const h = (z * Math.sqrt((p * (1 - p)) / n + (z * z) / (4 * n * n))) / d;
      return [Math.max(0, c - h), Math.min(1, c + h)];
    };
    if (dim === "dist_bin") {
      const rows: (string | number | null)[][] = [];
      for (let lo = 0; lo < 600; lo += 50) {
        const p = clamp(base.mean * (1 - ((lo + 25) / 700) ** 2));
        const n = Math.max(1, Math.round(base.n * 40 * (1 - lo / 900)));
        const [a, b] = ratio ? wilson(p, n) : [null, null];
        rows.push([`${lo}-${lo + 50}`, p, a, b, n]);
      }
      return rows;
    }
    if (dim === "node") {
      return nodes.map((node) => {
        const v = clamp(base.mean * (0.7 + 0.6 * unit(`${name}:${node}`)));
        const n = Math.max(1, Math.round(base.n * (5 + 20 * unit(`n:${node}`))));
        const [a, b] = ratio ? wilson(v, n) : [null, null];
        return [String(node), v, a, b, n];
      });
    }
    if (dim === "msg_type") {
      return [
        ["bsm", clamp(base.mean * 0.9), null, null, base.n * 10],
        ["cam", clamp(base.mean * 1.15), null, null, base.n * 4],
      ];
    }
    return [];
  }
}
