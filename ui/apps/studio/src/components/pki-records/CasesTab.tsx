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
import { beatNow, caseHeadline, caseSteps, storyBeats, STAGE_HELP, STAGE_STORY } from "../../lib/pkiCases.js";
import { deviceName } from "../../lib/pkiCert.js";
import { GlossText } from "../pki/GlossText.js";
import { Term } from "../pki/Term.js";
import { VehicleName } from "../pki/VehicleName.js";
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
        <p
          className="pkir-pad dim"
          title="A case follows one misbehaving sender from the first report to the moment other vehicles refuse its messages: reported, decided, traced without naming it, put on the revocation list, enforced."
        >
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

/** The case's four beats as a labelled bar: the same words as the story on the right. */
function Progress({ c }: { readonly c: CaseView }): React.JSX.Element {
  const beats = storyBeats(c);
  const refused = c.stages.some((s) => s.stage === "refused");
  return (
    <span className="pkir-progress pki-beats" data-testid="pki-case-progress">
      {beats.map((b) => (
        <span key={b.key} className={b.done ? "on" : refused ? "off refused" : "off"} data-beat={b.key}>
          <i aria-hidden="true" />
          <small>{b.title}</small>
        </span>
      ))}
    </span>
  );
}

function CaseRow({ c, selected, onSelect }: { readonly c: CaseView; readonly selected: boolean; readonly onSelect: () => void }): React.JSX.Element {
  const col = boardColumn(c);
  const label = beatNow(c);
  const enf = enforcement(c);
  const head = caseHeadline(c);
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
          <span className="faint">{c.provider}</span>
          <span className="grow" />
          <span className={`pkir-stage-pill ${col ?? ""}`} title={col ? `MA stage: ${BOARD_COLUMNS.find((b) => b.stage === col)?.label ?? col}` : undefined}>
            {label}
          </span>
        </span>
        <Progress c={c} />
        <span className="pkir-case-numbers">
          {head.toList !== null ? (
            <span data-testid="pki-case-to-list">
              on the list <b className="mono">{secondsText(head.toList)}</b> after the first report
            </span>
          ) : head.refused ? (
            <span>not revoked</span>
          ) : (
            <span className="faint">not on a list yet</span>
          )}
          {enf ? (
            <span data-testid="pki-case-refusing">
              {" "}
              · <b className="mono">{enf.installed}</b> of {enf.of} vehicles refuse it
            </span>
          ) : null}
        </span>
        <span className="faint">
          {c.truth ? (
            <span data-testid="pki-case-parties">
              accused {c.truth.accused.map((a) => deviceName(a.device, a.node)).join(", ") || "unknown"} · {c.truth.reporters.length} reporter
              {c.truth.reporters.length === 1 ? "" : "s"}
            </span>
          ) : (
            `${c.steps.length} step${c.steps.length === 1 ? "" : "s"} seen by this view`
          )}
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
        {caseName(c)} <span className="dim" title={`Decided by the Misbehaviour Authority (MA) of ${c.provider}`}>· {c.provider} MA</span>
      </h3>
      {first !== undefined && last !== undefined && last - first >= 5e7 ? (
        <p className="dim" data-testid="pki-case-took">
          <b className="mono">{secondsText(last - first)}</b> from the first report to “{beatNow(c)}”
        </p>
      ) : null}
      <Story c={c} />
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
              <b key={a.device}>
                <VehicleName device={a.device} node={a.node} testId="pki-case-accused" />
                {a.provider ? ` (${a.provider})` : ""}{" "}
              </b>
            ))}
          </span>
          <span className="faint" title={c.truth.reporters.map((r) => `${deviceName(r.device, r.node)}, device ${r.device}${r.provider ? ` (${r.provider})` : ""}: ${r.reports} report${r.reports === 1 ? "" : "s"}`).join(", ")}>
            reported by {c.truth.reporters.length} vehicle{c.truth.reporters.length === 1 ? "" : "s"}
          </span>
        </div>
      ) : null}
      <ol className="pkir-steps" data-testid="pki-case-stages">
        {steps.map((s) => (
          <li key={s.stage} className={s.reached ? "reached" : "pending"} data-stage={s.stage}>
            <div className="pkir-step-marker" aria-hidden="true" />
            <div className="pkir-step-body">
              <div className="pkir-step-head">
                <b title={[STAGE_STORY[s.stage]?.says, STAGE_HELP[s.stage]?.term].filter(Boolean).join("\n\n")}>{STAGE_STORY[s.stage]?.title ?? s.label}</b>
                {s.reached && s.t !== null ? (
                  s.waitNs === null ? (
                    <span className="mono faint">at {secondsText(s.t)}</span>
                  ) : (
                    <span className="mono faint" title={`at ${secondsText(s.t)}`}>
                      {s.waitNs < 5e7 ? "at once" : `+${secondsText(s.waitNs)}`}
                    </span>
                  )
                ) : (
                  <span className="faint">not reached</span>
                )}
              </div>
              {s.detail ? <div className="faint pkir-step-detail">{s.detail}</div> : null}
              {s.learned.length > 0 ? (
                <details className="pki-learned-fold">
                  <summary className="faint" data-testid="pki-case-learned-toggle">
                    Who learns what ({s.learned.length})
                  </summary>
                  <ul className="pkir-learned" data-testid="pki-case-learned">
                    {s.learned.map((l, k) => (
                      <li key={k}>
                        <b>{l.who}</b> learns <GlossText text={l.what} />
                      </li>
                    ))}
                  </ul>
                </details>
              ) : null}
              {s.messages.length > 0 ? <StepMessages messages={s.messages} /> : null}
            </div>
          </li>
        ))}
      </ol>
      {Array.isArray(subjects) ? (
        <p className="faint" data-testid="pki-case-subjects">
          The accused pseudonym certificate{subjects.length === 1 ? "" : "s"}:{" "}
          {subjects.map((x: { i?: number; lv?: string }, k: number) => (
            <span key={k}>
              {k > 0 ? "; " : ""}
              <Term k="ij">week {x.i}</Term>, <Term k="lv">linkage value</Term> <span className="mono">{String(x.lv ?? "").slice(0, 12)}…</span>
            </span>
          ))}
        </p>
      ) : null}
      <details>
        <summary className="dim">Every investigation step ({c.steps.length})</summary>
        <table className="pkir-table compact" data-testid="pki-case-steps">
          <tbody>
            {c.steps.map((s) => (
              <tr key={s.id} className={`step-${s.kind}`}>
                <td className="mono">{secondsText(s.t)}</td>
                <td>{s.kind === "msg" ? `${entityLabel(s.from ?? "")} → ${entityLabel(s.to ?? "")}` : `${entityLabel(s.from ?? "")} records a stage`}</td>
                <td>{s.name.replace(/[-_]+/g, " ")}</td>
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
    <details className="pki-step-msgs">
      <summary className="faint">
        {messages.length} message{messages.length === 1 ? "" : "s"} between the authorities
      </summary>
      <StepMessageList messages={messages} />
    </details>
  );
}

function StepMessageList({ messages }: { readonly messages: readonly CaseStep[] }): React.JSX.Element {
  return (
    <ul className="pkir-step-msgs">
      {messages.map((m) => (
        <li key={m.id} title={STEP_LEARNS[m.name] ?? m.name}>
          <span className="mono faint">{secondsText(m.t)}</span> {entityLabel(m.from ?? "")} → {entityLabel(m.to ?? "")}: <span>{m.name.replace(/-/g, " ")}</span>
          {m.bytes ? <span className="faint mono"> {m.bytes} B</span> : null}
        </li>
      ))}
    </ul>
  );
}


/**
 * The case as four beats a reader takes in at once — reported, decided, on the list, refused —
 * each with the time since the first report, and how many devices refuse the sender now.
 */
function Story({ c }: { readonly c: CaseView }): React.JSX.Element {
  const beats = storyBeats(c);
  const t0 = stageTimes(c).find((s) => s.stage === "reports-in")?.t ?? stageTimes(c)[0]?.t ?? null;
  // Each beat at its simulated time, and how long it took after the beat before: where the time went.
  let prev: number | null = null;
  return (
    <ol className="pki-story" data-testid="pki-case-story" aria-label="The case in four steps">
      {beats.map((b) => {
        const at = b.done && b.at !== null && t0 !== null ? t0 + b.at : null;
        const step = at !== null && prev !== null ? at - prev : null;
        if (at !== null) prev = at;
        return (
          <li key={b.key} className={b.done ? "done" : "todo"} data-beat={b.key}>
            <span className="pki-story-dot" aria-hidden="true" />
            <b>{b.title}</b>
            <span className="mono" data-testid="pki-story-at">
              {at === null ? "not yet" : `at ${secondsText(at)}`}
            </span>
            <span className="faint mono" data-testid="pki-story-step">
              {at === null || step === null ? "" : step < 5e7 ? "at once" : `+${secondsText(step)}`}
            </span>
            {b.note ? <span className="faint">{b.note}</span> : null}
          </li>
        );
      })}
    </ol>
  );
}
