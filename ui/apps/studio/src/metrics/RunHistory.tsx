/**
 * The Runs tab's run history: the current run and the runs the engine ran before it in this
 * session (`runs.list`, at most eight), with any two of them compared in one click. "Previous"
 * is the run before the current one; older runs are named by their id.
 */

import { useCallback, useEffect, useState } from "react";
import type { RunDigest, RunHeader } from "@vwp/protocol";

import { engine } from "../state/engine.js";
import { useStudio } from "../state/store.js";
import { download } from "./exporting.js";
import { openRunCompare } from "./RunCompare.js";
import { sideText } from "./runCompareModel.js";

export function RunHistory(): React.JSX.Element | null {
  const state = useStudio((s) => s.run.state);
  const generation = useStudio((s) => s.run.generation);
  const [runs, setRuns] = useState<RunHeader[] | null>(null);
  const [a, setA] = useState<string>("previous");
  const [b, setB] = useState<string>("current");

  const refresh = useCallback(() => {
    engine
      .request("runs.list", {})
      .then((r) => setRuns(r.runs))
      .catch(() => setRuns(null));
  }, []);
  useEffect(refresh, [refresh, state, generation]);

  if (runs === null) return null;
  const refOf = (r: RunHeader): string => r.ref ?? r.run_id;
  const save = (ref: string): void => {
    void engine.request("runs.digest", { run: ref }).then((d: RunDigest) => {
      download(`run-${d.run_id.slice(0, 8)}.digest.json`, new Blob([JSON.stringify(d, null, 1)], { type: "application/json" }));
    });
  };

  return (
    <div className="section" data-testid="run-history">
      <h3>Runs this session ({runs.length})</h3>
      <p className="help">
        The engine keeps a summary of each run it ran (settings, seed, every metric). Pick a baseline (A) and a run to compare (B). Save a run to compare it later with{" "}
        <code>v2xw compare</code>.
      </p>
      {runs.length === 0 ? (
        <p className="dim">No run has produced a metric yet.</p>
      ) : (
        <table className="rcmp-table rcmp-history">
          <thead>
            <tr>
              <th>A</th>
              <th>B</th>
              <th>Run</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {runs.map((r) => (
              <tr key={r.run_id} data-testid={`run-history-${refOf(r)}`}>
                <td>
                  <input type="radio" name="rcmp-a" aria-label={`Baseline: ${r.run_id}`} checked={a === refOf(r)} onChange={() => setA(refOf(r))} />
                </td>
                <td>
                  <input type="radio" name="rcmp-b" aria-label={`Compare: ${r.run_id}`} checked={b === refOf(r)} onChange={() => setB(refOf(r))} />
                </td>
                <td>
                  <strong>{r.ref === "current" ? "Current" : r.ref === "previous" ? "Previous" : r.run_id.slice(0, 8)}</strong> <span className="dim">{sideText(r)}</span>
                </td>
                <td>
                  <button type="button" className="small" onClick={() => save(refOf(r))} title="Download this run's digest (settings, seed, metrics) as JSON">
                    Save
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      <div className="row">
        <button type="button" disabled={runs.length < 2 || a === b} onClick={() => openRunCompare(a, b)} data-testid="run-history-compare">
          Compare A with B
        </button>
        <button type="button" className="small" onClick={refresh}>
          Refresh
        </button>
      </div>
    </div>
  );
}
