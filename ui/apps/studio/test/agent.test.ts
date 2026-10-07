import { describe, expect, it } from "vitest";

import { cellText, foldEvents, keyToPointer, notable, parseInline, parseMarkdown, progressFraction, type AgentEvent, type Finding } from "../src/lib/agent.js";

describe("the agent panel's view of a job", () => {
  const events: AgentEvent[] = [
    { kind: "plan", text: "Start from the grid, double the demand." },
    {
      kind: "changes",
      base: "phase1-grid",
      reason: "closest small scenario",
      changes: [{ key: "actors.vehicles.demand.rate_veh_per_h", before: 30, after: 60, why: "double" }],
      valid: true,
      errors: [],
    },
    { kind: "run-started", label: "double", run_id: "run-1", t_end_s: 60 },
    { kind: "progress", label: "double", t_s: 20, t_end_s: 60, actors: 12 },
    { kind: "progress", label: "double", t_s: 60, t_end_s: 60, actors: 14 },
    { kind: "analysed", label: "double", analysis: { findings: [] } },
    { kind: "tool", name: "run_prepared", ok: true },
    { kind: "tool", name: "compare_runs", ok: false },
    { kind: "done" },
  ];

  it("folds the events in order into plan, changes, progress and completion", () => {
    const v = foldEvents(events);
    expect(v.plans).toEqual(["Start from the grid, double the demand."]);
    expect(v.changes[0]?.changes[0]?.after).toBe(60);
    expect(v.runs).toHaveLength(1);
    expect(v.runs[0]).toMatchObject({ label: "double", runId: "run-1", tS: 60, actors: 14, analysed: true });
    expect(v.tools).toBe(2);
    expect(v.failedTools).toBe(1);
    expect(v.done).toBe(true);
    expect(v.report).toBeNull();
  });

  it("reports progress as a bounded fraction", () => {
    expect(progressFraction(30, 60)).toBe(0.5);
    expect(progressFraction(90, 60)).toBe(1);
    expect(progressFraction(5, 0)).toBe(0);
  });

  it("turns a dotted key into the pointer the settings window addresses", () => {
    expect(keyToPointer("radio.rat")).toBe("/radio/rat");
    expect(keyToPointer("/time/duration_s")).toBe("/time/duration_s");
  });

  it("lists only notable findings", () => {
    const f = (id: string, severity: Finding["severity"]): Finding => ({ id, layer: "channel", severity, title: id, evidence: [] });
    expect(notable([f("a.b", "info"), f("c.d", "warning")]).map((x) => x.id)).toEqual(["c.d"]);
  });

  it("formats table cells without inventing precision", () => {
    expect(cellText(null)).toBe("—");
    expect(cellText(60)).toBe("60");
    expect(cellText(["all"])).toBe('["all"]');
  });
});

describe("the overview's Markdown", () => {
  it("makes finding ids into references and never reads HTML", () => {
    const inl = parseInline("The channel ran hot [channel.congested] at **0.9**, see `cbr` <b>x</b>");
    expect(inl).toContainEqual({ t: "finding", id: "channel.congested" });
    expect(inl).toContainEqual({ t: "bold", s: "0.9" });
    expect(inl).toContainEqual({ t: "code", s: "cbr" });
    expect(inl.some((p) => p.t === "text" && p.s.includes("<b>x</b>"))).toBe(true);
  });

  it("does not take a Markdown link for a finding", () => {
    expect(parseInline("[a.b](http://x)").some((p) => p.t === "finding")).toBe(false);
  });

  it("splits headings, list items and paragraphs", () => {
    const b = parseMarkdown("## Bottlenecks\n\n- one [traffic.delay]\n- two\n\nA paragraph\ncontinued.");
    expect(b.map((x) => x.t)).toEqual(["h", "li", "li", "p"]);
  });
});
