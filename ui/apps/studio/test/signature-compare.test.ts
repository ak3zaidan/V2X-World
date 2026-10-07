/**
 * The signature comparison's pure half (`src/metrics/sigCompareModel.ts`): a `comparison.json` is
 * read and refused by schema, a difference is better or worse by the row's direction, and the CSV
 * and Markdown exports carry every value with the baseline's differences.
 */

import { describe, expect, it } from "vitest";

import { groupOf } from "../src/metrics/model.js";
import { comparisonCsv, comparisonMarkdown, parseComparison, verdict, type Comparison } from "../src/metrics/sigCompareModel.js";

const doc: Comparison = {
  schema: "v2xw/signature-comparison/1",
  scenario: "Compare signature schemes",
  seed_hex: "0x01",
  duration_s: 30,
  baseline: "ecdsa-p256",
  schemes: [
    { id: "ecdsa-p256", label: "ECDSA P-256", mode: "classical", sig_octets: 66, key_octets: 33, cert_extra_octets: 0, quantum_level: 0, sign_engine: "cpu", verify_engine: "cpu", run_id: "c0000" },
    { id: "ml-dsa-44", label: "ML-DSA-44", mode: "post-quantum", sig_octets: 2420, key_octets: 1312, cert_extra_octets: 3633, quantum_level: 2, sign_engine: "cpu", verify_engine: "cpu", run_id: "c0001" },
  ],
  rows: [
    { key: "sig_signature_bytes", label: "Signature per message", group: "Bytes on the air", unit: "B/msg", better: "lower", values: [66, 2420], delta: [0, 2354], rel: [0, 35.67], level: [null, null], highlight: [false, true] },
    { key: "sig_neighbour_capacity", label: "Neighbours", group: "Device cost", unit: "count", better: "higher", values: [158, 141], delta: [0, -17], rel: [0, -0.11], level: ["healthy", "caution"], highlight: [false, true] },
  ],
  caveats: ["One realisation."],
};

describe("signature comparison", () => {
  it("reads its own schema and refuses another", () => {
    expect(parseComparison(JSON.stringify(doc)).schemes).toHaveLength(2);
    expect(() => parseComparison(JSON.stringify({ ...doc, schema: "other" }))).toThrow(/signature comparison/);
  });

  it("calls a difference better or worse by the row's direction", () => {
    expect(verdict(doc.rows[0]!, 1)).toBe("worse");
    expect(verdict(doc.rows[1]!, 1)).toBe("worse");
    expect(verdict(doc.rows[0]!, 0)).toBeNull();
  });

  it("exports every (row, scheme) to CSV and bolds the call-outs in Markdown", () => {
    const csv = comparisonCsv(doc).trim().split("\n");
    expect(csv).toHaveLength(1 + 2 * 2);
    expect(csv[2]).toContain("ml-dsa-44,post-quantum,2420,2354,35.67,,true");
    const md = comparisonMarkdown(doc);
    expect(md).toContain("**2420 (+3567 %)**");
    expect(md).toContain("## Device cost");
    expect(groupOf("sig_cbr_extra")).toBe("signing");
  });
});
