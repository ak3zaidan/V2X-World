import { describe, expect, it } from "vitest";

import {
  alertLine,
  cellText,
  chipLine,
  clock,
  isAlert,
  foldEvents,
  keyToPointer,
  mergeObservations,
  notable,
  observationsOf,
  parseInline,
  parseMarkdown,
  progressFraction,
  unseenCount,
  type AgentEvent,
  type Finding,
  type Observation,
} from "../src/lib/agent.js";

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

describe("the always-on watcher's feed", () => {
  const obs = (seq: number, run: string, severity: Observation["severity"] = "info"): Observation => ({
    seq,
    run_id: run,
    generation: run === "r1" ? 1 : 2,
    t_s: seq,
    severity,
    kind: "finding",
    text: `o${seq}`,
    evidence: [],
  });

  it("merges pages by seq, so a page read twice shows each line once", () => {
    const a = mergeObservations([], [obs(1, "r1"), obs(2, "r1")]);
    const b = mergeObservations(a, [obs(2, "r1"), obs(3, "r1")]);
    expect(b.map((o) => o.seq)).toEqual([1, 2, 3]);
    expect(mergeObservations(b, [obs(1, "r1")]).map((o) => o.seq)).toEqual([1, 2, 3]);
  });

  it("keeps one run's lines apart from the next run's", () => {
    const all = [obs(1, "r1"), obs(2, "r1"), obs(3, "r2")];
    expect(observationsOf(all, "r2", 2).map((o) => o.seq)).toEqual([3]);
    expect(observationsOf(all, null, null)).toEqual([]);
  });

  it("counts only unseen warnings for the header badge", () => {
    const all = [obs(1, "r1", "warning"), obs(2, "r1", "info"), obs(3, "r1", "critical"), obs(4, "r1", "notice")];
    expect(unseenCount(all, 0)).toBe(2);
    expect(unseenCount(all, 1)).toBe(1);
    expect(unseenCount(all, 4)).toBe(0);
  });

  it("writes simulated time as minutes and seconds", () => {
    expect(clock(65.9)).toBe("1:05");
    expect(clock(0)).toBe("0:00");
    expect(clock(Number.NaN)).toBe("0:00");
  });
});

describe("the minimal panel's chips and alerts", () => {
  const obs = (kind: Observation["kind"], severity: Observation["severity"], text: string): Observation => ({
    seq: 1,
    run_id: "r",
    generation: 1,
    t_s: 12,
    severity,
    kind,
    text,
    evidence: [],
  });
  it("folds a setup into one chip line", () => {
    const v = foldEvents([
      {
        kind: "staged",
        setup: 1,
        base: "current",
        chips: [
          { key: "city", label: "City", value: "Portland, OR" },
          { key: "radio.rat", label: "Radio technology", value: "DSRC" },
        ],
      },
      { kind: "done" },
    ]);
    expect(v.staged).toHaveLength(1);
    expect(chipLine(v.staged[0]!.chips)).toBe("City → Portland, OR · Radio technology → DSRC");
  });
  it("shows problems and events as alerts, not the watching or cleared lines", () => {
    expect(isAlert(obs("started", "info", "Watching this run."))).toBe(false);
    expect(isAlert(obs("cleared", "notice", "Cleared."))).toBe(false);
    expect(isAlert(obs("finding", "warning", "Congestion is building."))).toBe(true);
    expect(isAlert(obs("trend", "notice", "Attacks detected."))).toBe(true);
    expect(isAlert(obs("finished", "info", "The run ended."))).toBe(true);
    expect(isAlert(obs("finding", "notice", "A small thing."))).toBe(false);
  });
  it("keeps an alert to its first sentence, numbers intact", () => {
    expect(alertLine("Delivery is dropping: 0.82 of messages were received (3GPP TR 37.885). More words.")).toBe(
      "Delivery is dropping: 0.82 of messages were received (3GPP TR 37.885).",
    );
  });
});
