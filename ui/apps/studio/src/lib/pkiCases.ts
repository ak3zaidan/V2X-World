/**
 * A misbehaviour case as a step diagram, report to enforcement (the workspace's Misbehaviour
 * section): the seven MA stages of `pki.case` in order, each with when it was reached, how long
 * it waited for the one before, the investigation messages that belong to it and who learned what
 * there.
 *
 * Stages and their times are the MA's own transitions; messages are the case's `pki.flow` steps,
 * placed under a stage by what they are (a PLV lookup, a seed request, a CRL append) or, for one
 * the table does not name, under the latest stage reached by its time. Pure (`test/pki-cases.test.ts`).
 */

import { evidenceWords } from "./pkiGlossary.js";
import { BOARD_COLUMNS, STEP_LEARNS, enforcement, entityLabel, stageTimes, type CaseStep, type CaseView } from "./pkiRecords.js";

/** The stage each investigation message belongs to, by its name. */
export const MESSAGE_STAGE: Readonly<Record<string, string>> = {
  "pca-lookup-request": "plv-lookup",
  "pca-lookup-response": "plv-lookup",
  "blocklist-request": "plv-lookup",
  "blocklist-response": "plv-lookup",
  "blocklist-notice": "plv-lookup",
  "same-device-request": "la-seeds",
  "same-device-response": "la-seeds",
  "seed-request": "la-seeds",
  "seed-response": "la-seeds",
  "crl-append": "crl-published",
  "crl-publish": "crl-published",
  "crl-broadcast": "crl-published",
};

/** What a recipient learns from the messages `STEP_LEARNS` does not describe. */
const MORE_LEARNS: Readonly<Record<string, string>> = {
  "same-device-request": "two linkage chain identifiers to compare, sealed for it",
  "same-device-response": "whether the reported certificates belong to one device, not whose they are",
};

/** Each stage's standard term, for the tooltip. */
export const STAGE_HELP: Readonly<Record<string, { readonly term: string }>> = {
  "reports-in": { term: "Misbehaviour reports received (SAE J3287 / IEEE 1609.2.1 §6.3.5.12)" },
  corroborated: { term: "Enough independent reports to act on" },
  decision: { term: "Revocation decision: an operator's review or the automatic rule" },
  "plv-lookup": { term: "The ACA maps the certificate to the RA's request and the linkage chain identifiers; the RA blocklists the enrolment" },
  "la-seeds": { term: "Each Linkage Authority releases its linkage seed at the revocation period (iRev)" },
  "crl-published": { term: "The CRL Generator signs and publishes the list" },
  enforced: { term: "Devices that installed the list drop the revoked device's messages" },
};

/**
 * Each stage as a step of a story a reader follows without the acronyms: a short title and the
 * one sentence of what happens there. The standard stage name stays beside it (`STAGE_HELP`).
 */
export const STAGE_STORY: Readonly<Record<string, { readonly title: string; readonly says: string }>> = {
  "reports-in": { title: "Vehicles report it", says: "Receivers that saw implausible messages send signed misbehaviour reports to the Misbehaviour Authority (MA)." },
  corroborated: { title: "Reports agree", says: "Enough independent reports point at the same pseudonym for the MA to act." },
  decision: { title: "The MA decides to revoke", says: "An operator's review, or the automatic rule, decides the sender is misbehaving." },
  "plv-lookup": {
    title: "Certificate traced",
    says: "The pseudonym CA (ACA) maps the certificate to its linkage chain and the RA blocks the enrolment certificate. No one learns who the vehicle is.",
  },
  "la-seeds": { title: "Linkage seeds released", says: "Each Linkage Authority releases its seed; together they cover all of the vehicle's certificates from that week on." },
  "crl-published": { title: "On the revocation list", says: "The CRL Generator signs a new certificate revocation list (CRL) carrying the seeds. The list is public." },
  enforced: { title: "Devices refuse it", says: "Vehicles that installed the list drop the revoked vehicle's messages." },
  refused: { title: "Not revoked", says: "The MA decided the reports did not justify revocation." },
};

/**
 * A case's headline numbers: time from the first report to the decision, to the list and to the
 * first refusing device, ns, each null until reached; and how many devices refuse it.
 */
export function caseHeadline(c: CaseView): {
  readonly toDecision: number | null;
  readonly toList: number | null;
  readonly toEnforced: number | null;
  readonly enforced: { installed: number; of: number } | null;
  readonly refused: boolean;
} {
  const times = new Map(stageTimes(c).map((s) => [s.stage, s.t]));
  const t0 = times.get("reports-in") ?? (times.size > 0 ? Math.min(...times.values()) : null);
  const since = (s: string): number | null => {
    const t = times.get(s);
    return t === undefined || t0 === null ? null : t - t0;
  };
  return { toDecision: since("decision"), toList: since("crl-published"), toEnforced: since("enforced"), enforced: enforcement(c), refused: times.has("refused") };
}

/** One stage of the diagram. */
export interface CaseStepRow {
  readonly stage: string;
  readonly label: string;
  readonly reached: boolean;
  readonly t: number | null;
  /** Time since the stage before it was reached, ns; null for the first or one not reached. */
  readonly waitNs: number | null;
  readonly detail: string;
  readonly learned: readonly { readonly who: string; readonly what: string }[];
  readonly messages: readonly CaseStep[];
}

/** The detail line of one stage, from the MA's record. */
function detailOf(c: CaseView, stage: string): string {
  const d = c.stages.find((s) => s.stage === stage)?.detail ?? {};
  switch (stage) {
    case "reports-in": {
      const ev = Array.isArray(d.evidence) ? d.evidence.map(evidencePlain).join(" · ") : "";
      return ev ? `What they saw: ${ev}` : "";
    }
    case "corroborated": {
      if (d.reports === undefined) return "";
      const n = Number(d.reports);
      // The count the MA's rule acted on, beside how many reports came in for the case in all (ground truth).
      const total = (c.truth?.reporters ?? []).reduce((s, r) => s + (r.reports ?? 0), 0);
      const vehicles = c.truth?.reporters.length ?? 0;
      return `${n} report${n === 1 ? " was" : "s were"} enough for the MA's rule to act${total > n ? `; more kept arriving: ${total} reports from ${vehicles} vehicle${vehicles === 1 ? "" : "s"} in all` : ""}`;
    }
    case "decision":
      return d.mode === "manual-review" ? "an operator reviewed it" : d.mode !== undefined ? `${String(d.mode)} rule` : "";
    case "plv-lookup":
      return d.lookups !== undefined ? `${String(d.lookups)} lookup${Number(d.lookups) === 1 ? "" : "s"}` : "";
    case "la-seeds":
      return d.i_rev !== undefined ? `revoked from week ${String(d.i_rev)} on · ${String(d.jmax ?? "?")} certificates a week` : "";
    case "crl-published":
      return typeof d.crl_instance === "string" ? `on the list built by ${d.crl_instance}` : "";
    case "enforced": {
      const e = enforcement(c);
      return e ? `${e.installed} of the ${e.of} vehicles in the run have installed the list and drop its messages` : "";
    }
    default:
      return "";
  }
}

/** What the MA's own stages teach, beyond the messages. */
function stageLearns(c: CaseView, stage: string): { who: string; what: string }[] {
  const ma = entityLabel(`${c.provider}/ma/0`);
  switch (stage) {
    case "reports-in":
      return [{ who: ma, what: "the reports, their evidence and the accused's pseudonym certificates and linkage values, not whose they are" }];
    case "decision": {
      const d = c.stages.find((s) => s.stage === "decision")?.detail ?? {};
      return [{ who: ma, what: d.mode === "manual-review" ? "that its operator decided to revoke" : "that its rule decided to revoke" }];
    }
    case "crl-published":
      return [{ who: "Every device", what: "the new list entry (the list is public)" }];
    case "enforced":
      return [{ who: "Each installing vehicle", what: "to drop the accused's messages; the MA is not told who installed the list" }];
    default:
      return [];
  }
}

/** The case's seven stages (and "refused" when the case ended there), in order, with their messages. */
export function caseSteps(c: CaseView): CaseStepRow[] {
  const times = new Map(stageTimes(c).map((s) => [s.stage, s.t]));
  const order: string[] = BOARD_COLUMNS.map((b) => b.stage);
  if (times.has("refused")) order.push("refused");
  const msgs = new Map<string, CaseStep[]>(order.map((s) => [s, []]));
  for (const m of c.steps) {
    if (m.kind !== "msg") continue;
    let stage = MESSAGE_STAGE[m.name];
    if (!stage || !msgs.has(stage)) {
      // The latest stage reached by its time, else the first.
      stage = order[0];
      for (const s of order) {
        const t = times.get(s);
        if (t !== undefined && t <= m.t) stage = s;
      }
    }
    msgs.get(stage)?.push(m);
  }
  let prev: number | null = null;
  return order.map((stage) => {
    const t = times.get(stage) ?? null;
    const waitNs = t !== null && prev !== null ? t - prev : null;
    if (t !== null) prev = t;
    const messages = msgs.get(stage) ?? [];
    const learned = [
      ...stageLearns(c, stage),
      ...messages.filter((m) => m.to).map((m) => ({ who: entityLabel(m.to ?? ""), what: STEP_LEARNS[m.name] ?? MORE_LEARNS[m.name] ?? m.name.replace(/-/g, " ") })),
    ];
    return {
      stage,
      label: BOARD_COLUMNS.find((b) => b.stage === stage)?.label ?? "Refused",
      reached: t !== null,
      t,
      waitNs,
      detail: detailOf(c, stage),
      learned: t !== null || messages.length > 0 ? learned : [],
      messages,
    };
  });
}

/** One beat of a case's four-step story. */
export interface StoryBeat {
  readonly key: "reports-in" | "decision" | "crl-published" | "enforced";
  readonly title: string;
  /** Time since the first report, ns; null until reached. */
  readonly at: number | null;
  readonly done: boolean;
  readonly note?: string;
}

/**
 * A case as the four beats a reader takes in at once — reported, the MA decided, on the list,
 * devices refuse it — each with the time since the first report. The case list, the case card
 * and the story use these same four words.
 */
export function storyBeats(c: CaseView): StoryBeat[] {
  const h = caseHeadline(c);
  const reached = new Set(c.stages.map((s) => s.stage));
  return [
    { key: "reports-in", title: "Reported", at: reached.has("reports-in") ? 0 : null, done: reached.has("reports-in") },
    { key: "decision", title: h.refused ? "MA decided: not revoked" : "MA decided", at: h.toDecision, done: reached.has("decision") || h.refused },
    { key: "crl-published", title: "On the list", at: h.toList, done: reached.has("crl-published") },
    {
      key: "enforced",
      title: "Devices refuse it",
      at: h.toEnforced,
      done: reached.has("enforced"),
      note: h.enforced ? `${h.enforced.installed} of ${h.enforced.of} vehicles` : undefined,
    },
  ];
}

/** The furthest beat reached, in the story's words ("Seen in steps only" when the view holds no stage). */
export function beatNow(c: CaseView): string {
  const done = storyBeats(c).filter((b) => b.done);
  return done.length === 0 ? "Seen in steps only" : done[done.length - 1].title;
}

/** One piece of evidence in words: what was wrong, and whether the MA could verify it. */
export function evidencePlain(e: unknown): string {
  const r = (e ?? {}) as Record<string, unknown>;
  if (typeof r.type_name === "string") {
    const subs = Array.isArray(r.subtypes) ? r.subtypes.map((s) => String((s as Record<string, unknown>).name)) : [];
    const what = evidenceWords([r.type_name, ...subs].join("/"));
    const verified =
      r.verified === true
        ? "the MA checked it independently"
        : r.verified === false
          ? "evidence the MA could not check independently, so it relied on how many reports agreed"
          : "not checked";
    return `${what}; ${verified}${r.shared === true ? "; shared by the other provider's MA" : ""}`;
  }
  if (typeof r.modelled === "string") {
    const checks = Array.isArray(r.checks) ? r.checks.join(", ") : "";
    return `a modelled detector (${r.modelled})${checks ? ` flagged: ${checks}` : ""}`;
  }
  return "evidence of an unknown kind";
}
