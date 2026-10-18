/**
 * The PKI view's Cases tab (00-gap-analysis §3.2): a board of misbehaviour cases, one column per
 * stage — Reports in → Corroborated → Decision (manual or automatic) → PLV lookup → LA seeds →
 * CRL published → Enforced (n of N vehicles installed) — and, for the chosen case, every
 * investigation step in order and who learned what from it.
 *
 * The columns are the MA's own `pki.case` transitions; the steps are the `pki.flow` messages and
 * stage stamps of the case's linkage-resolution and CRL-issuance instances. Seen as one entity
 * (the view selector) the board shows only the steps that entity sent or received and no
 * identity; the reporters and the accused, by device and provider, are ground truth only — the
 * MA itself holds linkage values and pseudonym certificates, never who they belong to.
 */

import {
  BOARD_COLUMNS,
  GROUND_TRUTH,
  asCases,
  boardColumn,
  enforcement,
  entityLabel,
  secondsText,
  stageTimes,
  whoLearned,
  type CaseView,
} from "../../lib/pkiRecords.js";
import { usePki, usePkiView } from "./state.js";
import { Locked } from "./VehiclesTab.js";

export function CasesTab({ view }: { readonly view: string | null }): React.JSX.Element {
  const caseId = usePkiView((s) => s.caseId);
  const set = usePkiView((s) => s.set);
  const answer = asCases(usePki(view ? { topic: "cases", view } : null).data);
  if (view === null) return <Locked />;
  const cases = answer?.cases ?? [];
  const chosen = cases.find((c) => c.case === caseId) ?? null;
  const columns = [...BOARD_COLUMNS.map((b) => b.stage as string), "refused"];
  return (
    <div className="pkir-cases" data-testid="pki-cases">
      {view !== GROUND_TRUTH ? (
        <p className="faint pkir-pad" data-testid="pki-cases-as">
          As {entityLabel(view)}: the steps it took part in{view.includes("/ma/") ? " and its own case table" : ""}; no reporter or accused is named.
        </p>
      ) : null}
      <div className="pkir-board" data-testid="pki-board">
        {columns.map((stage) => {
          const here = cases.filter((c) => (boardColumn(c) ?? "unstaged") === stage);
          const label = BOARD_COLUMNS.find((b) => b.stage === stage)?.label ?? "Refused";
          if (stage === "refused" && here.length === 0) return null;
          return (
            <div key={stage} className="pkir-col" data-testid="pki-board-col" data-stage={stage}>
              <div className="pkir-col-head">{label} <span className="faint">{here.length}</span></div>
              {here.map((c) => (
                <CaseCard key={c.case} c={c} selected={c.case === caseId} onSelect={() => set({ caseId: c.case })} />
              ))}
            </div>
          );
        })}
        {cases.some((c) => boardColumn(c) === null) ? (
          <div className="pkir-col" data-stage="unstaged">
            <div className="pkir-col-head">Seen in steps only</div>
            {cases.filter((c) => boardColumn(c) === null).map((c) => (
              <CaseCard key={c.case} c={c} selected={c.case === caseId} onSelect={() => set({ caseId: c.case })} />
            ))}
          </div>
        ) : null}
      </div>
      {cases.length === 0 ? <p className="dim pkir-pad">No misbehaviour case has been opened{view === GROUND_TRUTH ? "" : " that this entity took part in"}.</p> : null}
      {chosen ? <CaseDetail c={chosen} gt={view === GROUND_TRUTH} /> : null}
    </div>
  );
}

function CaseCard({ c, selected, onSelect }: { readonly c: CaseView; readonly selected: boolean; readonly onSelect: () => void }): React.JSX.Element {
  const enf = enforcement(c);
  const decision = c.stages.find((s) => s.stage === "decision");
  return (
    <button type="button" className={`pkir-case${selected ? " sel" : ""}`} onClick={onSelect} data-testid="pki-case-card" data-case={c.case}>
      <div className="mono">{c.case}</div>
      <div className="faint">
        {decision ? `${String(decision.detail.mode ?? "")} decision` : `${c.steps.length} steps`}
        {enf ? ` · enforced ${enf.installed} of ${enf.of}` : ""}
      </div>
      {c.truth ? (
        <div className="faint" data-testid="pki-case-parties">
          {c.truth.reporters.length} reporter{c.truth.reporters.length === 1 ? "" : "s"} · accused {c.truth.accused.map((a) => a.device).join(", ") || "unknown"}
        </div>
      ) : null}
    </button>
  );
}

function CaseDetail({ c, gt }: { readonly c: CaseView; readonly gt: boolean }): React.JSX.Element {
  const set = usePkiView((s) => s.set);
  const times = stageTimes(c);
  const learned = whoLearned(c);
  const subjects = c.stages.find((s) => s.stage === "reports-in")?.detail.subjects;
  return (
    <div className="pkir-card" data-testid="pki-case-detail">
      <h3 className="mono">{c.case} <span className="dim">· MA of {c.provider}</span></h3>
      {gt ? (
        <button type="button" className="pkir-link" onClick={() => set({ mapMode: "case" })} data-testid="pki-case-map">
          Show its reporters and accused on the map
        </button>
      ) : null}
      {times.length > 0 ? (
        <>
          <h4>Stages</h4>
          <table className="pkir-table compact" data-testid="pki-case-stages">
            <tbody>
              {times.map((s) => (
                <tr key={s.stage} data-stage={s.stage}>
                  <td>{BOARD_COLUMNS.find((b) => b.stage === s.stage)?.label ?? s.stage}</td>
                  <td className="mono">{secondsText(s.t)}</td>
                  <td className="mono faint">{s.dwellNs === null ? "" : `+${secondsText(s.dwellNs)}`}</td>
                  <td className="mono faint">{detailText(c.stages.find((x) => x.stage === s.stage)?.detail)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      ) : null}
      {Array.isArray(subjects) ? (
        <p className="faint mono">accused pseudonym(s): {subjects.map((s: { i?: number; lv?: string }) => `i ${s.i} · LV ${s.lv}`).join("; ")}</p>
      ) : null}
      {c.truth ? (
        <div data-testid="pki-case-truth">
          <h4>Ground truth</h4>
          <p>
            Accused:{" "}
            {c.truth.accused.map((a) => (
              <b key={a.device} className="mono">{a.device}{a.provider ? ` (${a.provider})` : ""} </b>
            ))}
          </p>
          <p>
            Reporters:{" "}
            {c.truth.reporters.map((r) => (
              <span key={r.device} className="mono">{r.device}{r.provider ? ` (${r.provider})` : ""} ×{r.reports} </span>
            ))}
          </p>
        </div>
      ) : null}
      <h4>Investigation steps</h4>
      <table className="pkir-table compact" data-testid="pki-case-steps">
        <tbody>
          {c.steps.map((s) => (
            <tr key={s.id} className={`step-${s.kind}`}>
              <td className="mono">{secondsText(s.t)}</td>
              <td>{s.kind === "msg" ? `${entityLabel(s.from ?? "")} → ${entityLabel(s.to ?? "")}` : `${entityLabel(s.from ?? "")} stamps`}</td>
              <td className="mono">{s.name}</td>
              <td className="mono faint">{s.bytes ? `${s.bytes} B` : ""}{s.pdus.length ? ` · ${s.pdus.map((p) => p.type).join(", ")}` : ""}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <h4>Who learned what</h4>
      <ul className="pkir-learned" data-testid="pki-case-learned">
        {learned.map((l, k) => (
          <li key={k}>
            <b>{entityLabel(l.who)}</b> <span className="faint">({secondsText(l.t)}, from {entityLabel(l.from)})</span>: {l.what}
          </li>
        ))}
      </ul>
    </div>
  );
}

function detailText(d: Readonly<Record<string, unknown>> | undefined): string {
  if (!d) return "";
  const parts: string[] = [];
  for (const [k, v] of Object.entries(d)) {
    if (k === "subjects") continue;
    parts.push(`${k} ${typeof v === "object" ? JSON.stringify(v) : String(v)}`);
  }
  return parts.join(" · ");
}
