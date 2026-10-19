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

import { BOARD_COLUMNS, STEP_LEARNS, enforcement, entityLabel, evidenceText, stageTimes, type CaseStep, type CaseView } from "./pkiRecords.js";

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
      const ev = Array.isArray(d.evidence) ? d.evidence.map(evidenceText).join("; ") : "";
      return ev ? `evidence ${ev}` : "";
    }
    case "corroborated":
      return d.reports !== undefined ? `${String(d.reports)} report${Number(d.reports) === 1 ? "" : "s"}` : "";
    case "decision":
      return d.mode === "manual-review" ? "an operator reviewed it" : d.mode !== undefined ? `${String(d.mode)} rule` : "";
    case "plv-lookup":
      return d.lookups !== undefined ? `${String(d.lookups)} lookup${Number(d.lookups) === 1 ? "" : "s"}` : "";
    case "la-seeds":
      return d.i_rev !== undefined ? `revoked from week i = ${String(d.i_rev)} · ${String(d.jmax ?? "?")} certificates a week` : "";
    case "crl-published":
      return typeof d.crl_instance === "string" ? d.crl_instance : "";
    case "enforced": {
      const e = enforcement(c);
      return e ? `installed by ${e.installed} device${e.installed === 1 ? "" : "s"} · ${e.of} in the run when last counted` : "";
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
