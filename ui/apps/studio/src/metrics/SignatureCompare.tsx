/**
 * "Compare signature schemes": one click runs the scenario on screen under several signature
 * schemes with the same seed (`POST /agent/compare-signatures`, which `v2xw serve` hosts), and the
 * result is shown side by side — the schemes' sizes and where they run, every signing-overhead
 * quantity with its difference against ECDSA P-256 (called-out ones highlighted, coloured by
 * whether they are better or worse), bar charts of the headline quantities — and exported as CSV or
 * Markdown. A `comparison.json` written by `v2xw compare-signatures` can be opened here too.
 */

import { useEffect, useRef, useState } from "react";

import { useStudio } from "../state/store.js";
import { download, downloadCsv } from "./exporting.js";
import {
  CHART_KEYS,
  DEFAULT_SELECTION,
  SIGNATURE_SCHEMES,
  comparisonCsv,
  comparisonMarkdown,
  formatRel,
  formatValue,
  parseComparison,
  verdict,
  type CompareJob,
  type Comparison,
  type ComparisonRow,
} from "./sigCompareModel.js";

const POLL_MS = 1000;

function endpoint(): string {
  const base = useStudio.getState().target.baseUrl;
  return `${base && base !== window.location.origin ? base.replace(/\/$/, "") : ""}/agent/compare-signatures`;
}

async function readJob(): Promise<CompareJob | null> {
  const r = await fetch(endpoint(), { headers: { "content-type": "application/json" } });
  if (!r.ok) throw new Error(`HTTP ${r.status}`);
  const body = (await r.json()) as { job: CompareJob | null };
  return body.job;
}

export function SignatureCompare(): React.JSX.Element {
  const scenario = useStudio((s) => s.scenario);
  const [selected, setSelected] = useState<readonly string[]>(DEFAULT_SELECTION);
  const [duration, setDuration] = useState(60);
  const [job, setJob] = useState<CompareJob | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<Comparison | null>(null);
  const timer = useRef<number | null>(null);

  const stopPolling = (): void => {
    if (timer.current !== null) window.clearInterval(timer.current);
    timer.current = null;
  };
  const poll = (): void => {
    stopPolling();
    timer.current = window.setInterval(() => {
      readJob()
        .then((j) => {
          setJob(j);
          if (j && !j.running) {
            stopPolling();
            if (j.result) setResult(j.result);
            if (j.error) setError(j.error);
          }
        })
        .catch((e: unknown) => {
          stopPolling();
          setError(`Lost the comparison: ${String(e)}`);
        });
    }, POLL_MS);
  };

  // Pick up a comparison already running or finished on this engine.
  useEffect(() => {
    readJob()
      .then((j) => {
        setJob(j);
        if (j?.running) poll();
        else if (j?.result) setResult(j.result);
      })
      .catch(() => undefined);
    return stopPolling;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const start = async (): Promise<void> => {
    setError(null);
    try {
      const r = await fetch(endpoint(), {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ document: scenario ?? undefined, schemes: selected, duration_s: duration }),
      });
      const body = (await r.json()) as CompareJob & { error?: string };
      if (!r.ok) {
        setError(body.error ?? `HTTP ${r.status}`);
        return;
      }
      setJob(body);
      poll();
    } catch (e) {
      setError(`The comparison needs the engine started with \`v2xw serve\`: ${String(e)}`);
    }
  };

  const open = async (file: File | undefined): Promise<void> => {
    if (!file) return;
    try {
      setResult(parseComparison(await file.text()));
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  };

  const toggle = (id: string): void =>
    setSelected((s) => (s.includes(id) ? s.filter((x) => x !== id) : [...s, id]));
  const running = job?.running === true;

  return (
    <section id="metrics-group-compare" className="mgroup sigcmp" data-testid="metrics-signature-compare">
      <h3>Compare signature schemes</h3>
      <p className="dim mgroup-q">
        The same scenario and seed under each scheme: what each way of signing costs on the air, in the devices and in the
        backend, against ECDSA P-256.
      </p>
      <div className="sigcmp-controls">
        <fieldset className="sigcmp-schemes" disabled={running}>
          <legend className="dim">Schemes</legend>
          {SIGNATURE_SCHEMES.map((s) => (
            <label key={s.id} title={`${s.mode}: ${s.id}`}>
              <input type="checkbox" checked={selected.includes(s.id)} onChange={() => toggle(s.id)} />
              {s.label}
            </label>
          ))}
        </fieldset>
        <label className="sigcmp-duration">
          Simulated seconds per scheme{" "}
          <input type="number" min={5} max={3600} value={duration} disabled={running} onChange={(e) => setDuration(Number(e.target.value) || 60)} />
        </label>
        <div className="sigcmp-actions">
          <button type="button" className="primary" disabled={running || selected.length === 0} onClick={() => void start()} data-testid="sigcmp-run">
            {running ? `Running… ${job?.done ?? 0} of ${job?.schemes.length ?? 0} schemes done` : "Run comparison"}
          </button>
          <label className="linklike sigcmp-open">
            Open comparison.json
            <input type="file" accept="application/json,.json" hidden onChange={(e) => void open(e.target.files?.[0])} />
          </label>
        </div>
      </div>
      {error ? <p className="sigcmp-error" role="alert">{error}</p> : null}
      {result ? <ComparisonView c={result} /> : null}
    </section>
  );
}

function ComparisonView({ c }: { c: Comparison }): React.JSX.Element {
  const base = Math.max(0, c.schemes.findIndex((s) => s.id === c.baseline));
  const charts = CHART_KEYS.map((k) => c.rows.find((r) => r.key === k)).filter((r): r is ComparisonRow => r !== undefined);
  const groups = [...new Set(c.rows.map((r) => r.group))];
  const stem = `signatures-${c.scenario.toLowerCase().replace(/[^a-z0-9]+/g, "-")}`;
  return (
    <div className="sigcmp-result" data-testid="sigcmp-result">
      <div className="sigcmp-actions">
        <span className="dim">
          {c.scenario} · seed {c.seed_hex} · {c.duration_s} s per scheme · differences against <strong>{c.baseline}</strong>
        </span>
        <button type="button" onClick={() => downloadCsv(`${stem}.csv`, comparisonCsv(c))}>
          Export CSV
        </button>
        <button type="button" onClick={() => download(`${stem}.md`, new Blob([comparisonMarkdown(c)], { type: "text/markdown;charset=utf-8" }))}>
          Export Markdown
        </button>
      </div>
      <table className="sigcmp-table">
        <thead>
          <tr>
            <th>Scheme</th>
            <th>Mode</th>
            <th>Signature B</th>
            <th>Key B</th>
            <th>Certificate +B</th>
            <th>Signs on</th>
            <th>Verifies on</th>
          </tr>
        </thead>
        <tbody>
          {c.schemes.map((s) => (
            <tr key={s.id}>
              <td>{s.id}</td>
              <td>{s.mode}</td>
              <td>{s.sig_octets}</td>
              <td>{s.key_octets}</td>
              <td>{s.cert_extra_octets}</td>
              <td>{s.sign_engine}</td>
              <td>{s.verify_engine}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <div className="mgrid sigcmp-charts">
        {charts.map((r) => (
          <BarChart key={r.key} row={r} c={c} base={base} />
        ))}
      </div>
      {groups.map((g) => (
        <table key={g} className="sigcmp-table">
          <thead>
            <tr>
              <th>{g}</th>
              <th>Unit</th>
              {c.schemes.map((s) => (
                <th key={s.id}>{s.id}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {c.rows
              .filter((r) => r.group === g)
              .map((r) => (
                <tr key={r.key}>
                  <td title={r.key}>{r.label}</td>
                  <td className="dim">{r.unit}</td>
                  {c.schemes.map((s, i) => {
                    const v = verdict(r, i);
                    const cls = [r.highlight[i] ? "hl" : "", v ?? "", r.level[i] ? `lv-${r.level[i]}` : ""].join(" ").trim();
                    return (
                      <td key={s.id} className={cls} title={r.level[i] ?? undefined}>
                        {formatValue(r.values[i] ?? null)}
                        {i !== base && r.rel[i] !== null ? <span className="rel"> {formatRel(r.rel[i] ?? null)}</span> : null}
                      </td>
                    );
                  })}
                </tr>
              ))}
          </tbody>
        </table>
      ))}
      <ul className="dim sigcmp-caveats">
        {c.caveats.map((x) => (
          <li key={x}>{x}</li>
        ))}
      </ul>
    </div>
  );
}

function BarChart({ row, c, base }: { row: ComparisonRow; c: Comparison; base: number }): React.JSX.Element {
  const max = Math.max(...row.values.map((v) => (v === null ? 0 : Math.abs(v))), 1e-12);
  return (
    <div className="mcard sigcmp-chart">
      <div className="mcard-head">
        <span className="mcard-title" title={row.key}>
          {row.label}
        </span>
        <span className="mcard-unit">{row.unit}</span>
      </div>
      <div className="mcard-bars">
        {c.schemes.map((s, i) => {
          const v = row.values[i] ?? null;
          const tone = i === base ? "base" : verdict(row, i) ?? "";
          return (
            <div key={s.id} className="mbar-row">
              <span className="mbar-key" title={s.id}>
                {s.id}
              </span>
              <span className="mbar-track">
                <span className={`mbar sigcmp-bar ${tone}`} style={{ width: `${v === null ? 0 : (Math.abs(v) / max) * 100}%` }} />
              </span>
              <span className="mbar-val">{formatValue(v)}</span>
            </div>
          );
        })}
      </div>
    </div>
  );
}
