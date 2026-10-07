/**
 * The agent harness, as the page sees it: the shapes `v2xw serve`'s `/agent/*` endpoints return
 * (crates/v2xw-cli/src/agent_host.rs), the events a job emits (`AgentEvent` in
 * crates/v2xw-copilot/src/agent.rs), and the pure functions that turn a job's events into what the
 * panel draws. No fetch and no React here, so all of it is unit-tested in Node.
 *
 * The page never holds a model key: the agent runs in the server process, which reads
 * ANTHROPIC_API_KEY from its own environment. What arrives here is the agent's words and the
 * deterministic analyst's numbers.
 */

/** `GET /agent/status`. */
export interface AgentStatus {
  readonly model: boolean;
  readonly provider: string;
  /** Where the key comes from, named (never the key). */
  readonly key_source: string | null;
  /** What to do when there is no model. */
  readonly how_to_add_key: string | null;
  readonly runs: number;
  readonly job: number;
  readonly running: boolean;
  readonly kind: string;
}

export type Severity = "info" | "notice" | "warning" | "critical";

export interface Evidence {
  readonly label: string;
  readonly value: string | number;
  readonly unit: string;
  readonly source: string;
}

export interface FindingLocation {
  readonly nodes?: readonly string[];
  readonly regions?: readonly string[];
  readonly entities?: readonly string[];
  readonly distance_bins?: readonly string[];
  readonly window_s?: readonly [number, number];
}

export interface FindingLink {
  readonly metric: string;
  readonly chart: string;
}

/** A finding as the report carries it (`Finding` in analyst.rs). */
export interface Finding {
  readonly id: string;
  readonly layer: string;
  readonly severity: Severity;
  readonly title: string;
  readonly detail?: string;
  readonly evidence: readonly Evidence[];
  readonly location?: FindingLocation;
  readonly where?: FindingLocation;
  readonly reference?: string | null;
  readonly links?: readonly FindingLink[];
}

export interface Change {
  readonly key: string;
  readonly before: unknown;
  readonly after: unknown;
  readonly why: string;
}

export interface RunRecord {
  readonly label: string;
  readonly t_s: number;
  readonly prepared: { readonly base: string; readonly reason: string; readonly changes: readonly Change[]; readonly scenario_hash: string } | null;
  readonly analysis: {
    readonly run_id: string;
    readonly scenario_hash: string;
    readonly findings: readonly Finding[];
    readonly headline: Readonly<Record<string, number>>;
    readonly not_measured: readonly string[];
  };
}

export interface Comparison {
  readonly deltas: readonly { readonly metric: string; readonly a: number; readonly b: number; readonly change: number }[];
  readonly only_in_a: readonly string[];
  readonly only_in_b: readonly string[];
}

export interface AgentReport {
  readonly prompt: string;
  readonly provider: string;
  readonly overview: string;
  readonly ungrounded: readonly string[];
  readonly runs: readonly RunRecord[];
  readonly comparison: Comparison | null;
  readonly stopped: boolean;
  readonly markdown: string;
}

/** One event of a job. `done` is added by the host when the job ends. */
export type AgentEvent =
  | { readonly kind: "status"; readonly text: string }
  | { readonly kind: "plan"; readonly text: string }
  | {
      readonly kind: "changes";
      readonly base: string;
      readonly reason: string;
      readonly changes: readonly Change[];
      readonly valid: boolean;
      readonly errors: readonly { readonly field: string | null; readonly message: string }[];
    }
  | { readonly kind: "run-started"; readonly label: string; readonly run_id: string; readonly t_end_s: number }
  | { readonly kind: "progress"; readonly label: string; readonly t_s: number; readonly t_end_s: number; readonly actors: number }
  | { readonly kind: "analysed"; readonly label: string; readonly analysis: { readonly findings: readonly Finding[] } }
  | { readonly kind: "tool"; readonly name: string; readonly ok: boolean }
  | { readonly kind: "report"; readonly report: AgentReport }
  | { readonly kind: "error"; readonly message: string }
  | { readonly kind: "done" };

/** `GET /agent/events`. */
export interface EventsPage {
  readonly job: number;
  readonly kind: string;
  readonly events: readonly AgentEvent[];
  readonly next: number;
  readonly running: boolean;
}

/** What the panel draws for one job, folded from its events. */
export interface JobView {
  readonly plans: readonly string[];
  readonly statuses: readonly string[];
  readonly changes: readonly Extract<AgentEvent, { kind: "changes" }>[];
  readonly runs: readonly { readonly label: string; readonly runId: string; readonly tS: number; readonly tEndS: number; readonly actors: number; readonly analysed: boolean }[];
  readonly tools: number;
  readonly failedTools: number;
  readonly report: AgentReport | null;
  readonly errors: readonly string[];
  readonly done: boolean;
}

export const EMPTY_JOB: JobView = {
  plans: [],
  statuses: [],
  changes: [],
  runs: [],
  tools: 0,
  failedTools: 0,
  report: null,
  errors: [],
  done: false,
};

/** Folds a job's events into what the panel draws. Order-preserving and idempotent per event. */
export function foldEvents(events: readonly AgentEvent[]): JobView {
  const plans: string[] = [];
  const statuses: string[] = [];
  const changes: Extract<AgentEvent, { kind: "changes" }>[] = [];
  const runs: { label: string; runId: string; tS: number; tEndS: number; actors: number; analysed: boolean }[] = [];
  let tools = 0;
  let failedTools = 0;
  let report: AgentReport | null = null;
  const errors: string[] = [];
  let done = false;
  const runFor = (label: string) => {
    for (let i = runs.length - 1; i >= 0; i--) if (runs[i]!.label === label) return runs[i]!;
    const r = { label, runId: "", tS: 0, tEndS: 0, actors: 0, analysed: false };
    runs.push(r);
    return r;
  };
  for (const e of events) {
    switch (e.kind) {
      case "status":
        statuses.push(e.text);
        break;
      case "plan":
        plans.push(e.text);
        break;
      case "changes":
        changes.push(e);
        break;
      case "run-started": {
        const r = { label: e.label, runId: e.run_id, tS: 0, tEndS: e.t_end_s, actors: 0, analysed: false };
        runs.push(r);
        break;
      }
      case "progress": {
        const r = runFor(e.label);
        r.tS = e.t_s;
        r.tEndS = e.t_end_s;
        r.actors = e.actors;
        break;
      }
      case "analysed":
        runFor(e.label).analysed = true;
        break;
      case "tool":
        tools += 1;
        if (!e.ok) failedTools += 1;
        break;
      case "report":
        report = e.report;
        break;
      case "error":
        errors.push(e.message);
        break;
      case "done":
        done = true;
        break;
    }
  }
  return { plans, statuses, changes, runs, tools, failedTools, report, errors, done };
}

/** A run's progress as a fraction in [0, 1]. */
export function progressFraction(tS: number, tEndS: number): number {
  if (!(tEndS > 0)) return 0;
  return Math.max(0, Math.min(1, tS / tEndS));
}

/** A dotted scenario key as a JSON Pointer (what the settings window addresses fields by). */
export function keyToPointer(key: string): string {
  if (key.startsWith("/")) return key;
  return `/${key.split(".").map((p) => p.replace(/~/g, "~0").replace(/\//g, "~1")).join("/")}`;
}

/** A value for a table cell. */
export function cellText(v: unknown): string {
  if (v === null || v === undefined) return "—";
  if (typeof v === "string") return v;
  if (typeof v === "number") return Number.isInteger(v) ? String(v) : String(Number(v.toPrecision(6)));
  return JSON.stringify(v);
}

/** The findings worth listing: notice and above, most severe first (the analyst already sorts). */
export function notable(findings: readonly Finding[]): Finding[] {
  return findings.filter((f) => f.severity !== "info");
}

/** One inline piece of the overview's Markdown. */
export type Inline =
  | { readonly t: "text"; readonly s: string }
  | { readonly t: "bold"; readonly s: string }
  | { readonly t: "code"; readonly s: string }
  | { readonly t: "finding"; readonly id: string };

/** One block of the overview's Markdown. */
export type Block =
  | { readonly t: "h"; readonly level: number; readonly inl: readonly Inline[] }
  | { readonly t: "p"; readonly inl: readonly Inline[] }
  | { readonly t: "li"; readonly inl: readonly Inline[] }
  | { readonly t: "quote"; readonly inl: readonly Inline[] };

const FINDING_ID = /^[a-z]+(?:\.[a-z0-9-]+)+$/;

/**
 * The inline Markdown the overview uses: **bold**, `code`, and [layer.finding-id] references,
 * which the panel turns into links to the finding. Anything else is text — nothing is ever
 * interpreted as HTML.
 */
export function parseInline(s: string): Inline[] {
  const out: Inline[] = [];
  let i = 0;
  let text = "";
  const flush = () => {
    if (text !== "") out.push({ t: "text", s: text });
    text = "";
  };
  while (i < s.length) {
    const rest = s.slice(i);
    if (rest.startsWith("**")) {
      const end = s.indexOf("**", i + 2);
      if (end > i + 2) {
        flush();
        out.push({ t: "bold", s: s.slice(i + 2, end) });
        i = end + 2;
        continue;
      }
    }
    if (rest.startsWith("`")) {
      const end = s.indexOf("`", i + 1);
      if (end > i + 1) {
        flush();
        const code = s.slice(i + 1, end);
        out.push(FINDING_ID.test(code) ? { t: "finding", id: code } : { t: "code", s: code });
        i = end + 1;
        continue;
      }
    }
    if (rest.startsWith("[")) {
      const end = s.indexOf("]", i + 1);
      const inner = end > i ? s.slice(i + 1, end) : "";
      if (end > i && FINDING_ID.test(inner) && s[end + 1] !== "(") {
        flush();
        out.push({ t: "finding", id: inner });
        i = end + 1;
        continue;
      }
    }
    text += s[i];
    i += 1;
  }
  flush();
  return out;
}

/** The overview's Markdown as blocks: headings, paragraphs, list items, quotes. */
export function parseMarkdown(md: string): Block[] {
  const blocks: Block[] = [];
  let para: string[] = [];
  const flush = () => {
    if (para.length > 0) blocks.push({ t: "p", inl: parseInline(para.join(" ")) });
    para = [];
  };
  for (const raw of md.split("\n")) {
    const line = raw.trimEnd();
    const h = /^(#{1,4})\s+(.*)$/.exec(line);
    const li = /^\s*(?:[-*]|\d+\.)\s+(.*)$/.exec(line);
    if (line.trim() === "") {
      flush();
    } else if (h) {
      flush();
      blocks.push({ t: "h", level: h[1]!.length, inl: parseInline(h[2]!) });
    } else if (li) {
      flush();
      blocks.push({ t: "li", inl: parseInline(li[1]!) });
    } else if (line.startsWith(">")) {
      flush();
      blocks.push({ t: "quote", inl: parseInline(line.replace(/^>\s?/, "")) });
    } else {
      para.push(line.trim());
    }
  }
  flush();
  return blocks;
}

/** The prompts the empty panel offers, each one the harness can carry out end to end. */
export const SUGGESTIONS: readonly string[] = [
  "How did the run on screen do? Where are the bottlenecks?",
  "Run a busy signalised grid at 1,500 vehicles per hour and tell me where the network struggles.",
  "Compare DSRC with LTE-V2X on the same scenario.",
  "Run the credential lifecycle and check pseudonym top-ups and revocation.",
];
