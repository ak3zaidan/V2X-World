/**
 * The workspace's Misbehaviour section (00-gap-analysis §3.2 Cases): every misbehaviour case as a
 * row with its progress through the seven stages — Reports in → Corroborated → Decision (manual
 * or automatic) → PLV lookup → LA seeds → CRL published → Enforced — and, for the chosen case, a
 * step diagram from report to enforcement: when each stage was reached, how long it waited, the
 * messages of the investigation under the stage they belong to, and who learned what at each.
 *
 * The stages are the MA's own `pki.case` transitions; the steps are the `pki.flow` messages and
 * stage stamps of the case's linkage-resolution and CRL-issuance instances. Seen as one entity
 * (the view selector) the section shows only the steps that entity sent or received and no
 * identity; the reporters and the accused, by device and provider, are ground truth only — the
 * MA itself holds linkage values and pseudonym certificates, never who they belong to.
 */

import {
  BOARD_COLUMNS,
  GROUND_TRUTH,
  STEP_LEARNS,
  asCases,
  boardColumn,
  enforcement,
  entityLabel,
  secondsText,
  stageTimes,
  type CaseStep,
  type CaseView,
} from "../../lib/pkiRecords.js";
import { caseSteps, STAGE_HELP } from "../../lib/pkiCases.js";
import { usePki, usePkiView } from "./state.js";
import { Locked } from "./VehiclesTab.js";

export function CasesTab({ view }: { readonly view: string | null }): React.JSX.Element {
  const caseId = usePkiView((s) => s.caseId);
  const set = usePkiView((s) => s.set);
  const answer = asCases(usePki(view ? { topic: "cases", view } : null).data);
  if (view === null) return <Locked />;
  const cases = answer?.cases ?? [];
  const chosen = cases.find((c) => c.case === caseId) ?? cases[cases.length - 1] ?? null;
  const open = cases.filter((c) => {
    const col = boardColumn(c);
    return col !== "enforced" && col !== "refused";
  }).length;
  return (
    <div className="pkir-split pkir-cases" data-testid="pki-cases">
      <div className="pkir-table-wrap">
        {view !== GROUND_TRUTH ? (
          <p className="faint pkir-pad" data-testid="pki-cases-as">
            As {entityLabel(view)}: the steps it took part in{view.includes("/ma/") ? " and its own case table" : ""}; no reporter or accused is named.
          </p>
        ) : null}
        <p className="pkir-pad dim">
          {cases.length} case{cases.length === 1 ? "" : "s"} · {open} open
        </p>
        <ol className="pkir-caselist" data-testid="pki-board">
          {[...cases].reverse().map((c) => (
            <CaseRow key={c.case} c={c} selected={chosen?.case === c.case} onSelect={() => set({ caseId: c.case })} />
          ))}
        </ol>
        {cases.length === 0 ? <p className="dim pkir-pad">No misbehaviour case has been opened{view === GROUND_TRUTH ? "" : " that this entity took part in"}.</p> : null}
      </div>
      {chosen ? <CaseDetail c={chosen} gt={view === GROUND_TRUTH} /> : <div className="pkir-card dim">Choose a case.</div>}
    </div>
  );
}

/** The case's number and provider in plain words (`provider-b/case/235` → "Case 235"). */
function caseName(c: Pick<CaseView, "case">): string {
  const n = c.case.split("/").pop();
  return `Case ${n ?? c.case}`;
}

function Progress({ c }: { readonly c: CaseView }): React.JSX.Element {
  const reached = new Set(c.stages.map((s) => s.stage));
  const refused = reached.has("refused");
  return (
    <span className="pkir-progress" aria-hidden="true">
      {BOARD_COLUMNS.map((b) => (
        <i key={b.stage} className={reached.has(b.stage) ? "on" : refused ? "off refused" : "off"} title={b.label} />
      ))}
    </span>
  );
}

function CaseRow({ c, selected, onSelect }: { readonly c: CaseView; readonly selected: boolean; readonly onSelect: () => void }): React.JSX.Element {
  const col = boardColumn(c);
  const label = col === "refused" ? "Refused" : (BOARD_COLUMNS.find((b) => b.stage === col)?.label ?? "Seen in steps only");
  const enf = enforcement(c);
  return (
    <li>
      <button
        type="button"
        className={`pkir-case${selected ? " sel" : ""}`}
        onClick={onSelect}
        aria-pressed={selected}
        data-testid="pki-case-card"
        data-case={c.case}
        data-stage={col ?? "unstaged"}
      >
        <span className="pkir-case-head">
          <b>{caseName(c)}</b>
          <span className="faint mono">{c.provider}</span>
          <span className="grow" />
          <span className={`pkir-stage-pill ${col ?? ""}`}>{label}</span>
        </span>
        <Progress c={c} />
        <span className="faint">
          {c.truth ? (
            <span data-testid="pki-case-parties">
              accused {c.truth.accused.map((a) => a.device).join(", ") || "unknown"} · {c.truth.reporters.length} reporter{c.truth.reporters.length === 1 ? "" : "s"}
            </span>
          ) : (
            `${c.steps.length} steps`
          )}
          {enf ? ` · installed by ${enf.installed}` : ""}
        </span>
      </button>
    </li>
  );
}

function CaseDetail({ c, gt }: { readonly c: CaseView; readonly gt: boolean }): React.JSX.Element {
  const set = usePkiView((s) => s.set);
  const times = stageTimes(c);
  const steps = caseSteps(c);
  const subjects = c.stages.find((s) => s.stage === "reports-in")?.detail.subjects;
  const first = times[0]?.t;
  const last = times[times.length - 1]?.t;
  return (
    <div className="pkir-card" data-testid="pki-case-detail">
      <h3>
        {caseName(c)} <span className="dim mono">· decided by the MA of {c.provider}</span>
      </h3>
      {first !== undefined && last !== undefined ? (
        <p className="dim">
          report to {BOARD_COLUMNS.find((b) => b.stage === times[times.length - 1].stage)?.label.toLowerCase() ?? times[times.length - 1].stage} in{" "}
          <b className="mono">{secondsText(last - first)}</b>
        </p>
      ) : null}
      {gt ? (
        <button type="button" className="pkir-link" onClick={() => set({ mapMode: "case" })} data-testid="pki-case-map">
          Show its reporters and accused on the map
        </button>
      ) : null}
      {c.truth ? (
        <div className="pkir-truth" data-testid="pki-case-truth">
          <span className="faint">ground truth</span>
          <span>
            accused{" "}
            {c.truth.accused.map((a) => (
              <b key={a.device} className="mono">
                {a.device}
                {a.provider ? ` (${a.provider})` : ""}{" "}
              </b>
            ))}
          </span>
          <span className="faint" title={c.truth.reporters.map((r) => `${r.device}${r.provider ? ` (${r.provider})` : ""} ×${r.reports}`).join(", ")}>
            reported by {c.truth.reporters.length} device{c.truth.reporters.length === 1 ? "" : "s"}:{" "}
            {c.truth.reporters
              .slice(0, 6)
              .map((r) => `${r.device} ×${r.reports}`)
              .join(", ")}
            {c.truth.reporters.length > 6 ? ` +${c.truth.reporters.length - 6}` : ""}
          </span>
        </div>
      ) : null}
      <ol className="pkir-steps" data-testid="pki-case-stages">
        {steps.map((s) => (
          <li key={s.stage} className={s.reached ? "reached" : "pending"} data-stage={s.stage}>
            <div className="pkir-step-marker" aria-hidden="true" />
            <div className="pkir-step-body">
              <div className="pkir-step-head">
                <b title={STAGE_HELP[s.stage]?.term}>{s.label}</b>
                {s.reached && s.t !== null ? <span className="mono faint">{secondsText(s.t)}</span> : <span className="faint">not reached</span>}
                {s.waitNs !== null && s.waitNs > 0 ? <span className="mono faint">waited {secondsText(s.waitNs)}</span> : null}
              </div>
              {s.detail ? <div className="faint mono pkir-step-detail">{s.detail}</div> : null}
              {s.learned.length > 0 ? (
                <ul className="pkir-learned" data-testid="pki-case-learned">
                  {s.learned.map((l, k) => (
                    <li key={k}>
                      <b>{l.who}</b> learns {l.what}
                    </li>
                  ))}
                </ul>
              ) : null}
              {s.messages.length > 0 ? <StepMessages messages={s.messages} /> : null}
            </div>
          </li>
        ))}
      </ol>
      {Array.isArray(subjects) ? (
        <p className="faint mono">accused pseudonym(s): {subjects.map((x: { i?: number; lv?: string }) => `i ${x.i} · LV ${x.lv}`).join("; ")}</p>
      ) : null}
      <details>
        <summary className="dim">Every investigation step ({c.steps.length})</summary>
        <table className="pkir-table compact" data-testid="pki-case-steps">
          <tbody>
            {c.steps.map((s) => (
              <tr key={s.id} className={`step-${s.kind}`}>
                <td className="mono">{secondsText(s.t)}</td>
                <td>{s.kind === "msg" ? `${entityLabel(s.from ?? "")} → ${entityLabel(s.to ?? "")}` : `${entityLabel(s.from ?? "")} stamps`}</td>
                <td className="mono">{s.name}</td>
                <td className="mono faint">
                  {s.bytes ? `${s.bytes} B` : ""}
                  {s.pdus.length ? ` · ${s.pdus.map((p) => p.type).join(", ")}` : ""}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </details>
    </div>
  );
}

function StepMessages({ messages }: { readonly messages: readonly CaseStep[] }): React.JSX.Element {
  return (
    <ul className="pkir-step-msgs">
      {messages.map((m) => (
        <li key={m.id} title={STEP_LEARNS[m.name] ?? m.name}>
          <span className="mono faint">{secondsText(m.t)}</span> {entityLabel(m.from ?? "")} → {entityLabel(m.to ?? "")} <span className="mono">{m.name.replace(/-/g, " ")}</span>
          {m.bytes ? <span className="faint mono"> {m.bytes} B</span> : null}
        </li>
      ))}
    </ul>
  );
}

