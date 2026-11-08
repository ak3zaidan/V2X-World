/**
 * A vehicle's certificate lifecycle as a timeline (`lib/pkiVehicle.ts`): enrolment, the pseudonym
 * downloads and top-ups, the pseudonym it signs with and each change, the reports it sent and its
 * revocation, on the run's clock, with "now" marked and a tick every round interval; the factory
 * and dealer steps before the run are listed apart. Each event is focusable and names its flow
 * instance (the Messages section shows it whole); a pseudonym's bar opens its certificate.
 */

import { useEffect, useMemo, useState } from "react";

import { flowsOf, type FlowRow } from "../../lib/pki.js";
import { deviceName } from "../../lib/pkiCert.js";
import { secondsText, type VehicleDetail } from "../../lib/pkiRecords.js";
import { LANES, axisUnit, lifecycleOf, tickLabel, tickStep } from "../../lib/pkiVehicle.js";
import { engine } from "../../state/engine.js";
import { usePkiView } from "./state.js";

const W = 640;
const LANE_H = 22;
const LEFT = 92;
const PAD_R = 10;
const AXIS_H = 20;
/** Room above the lanes for the "now" label, so it never sits on a tick label. */
const TOP = 14;
/** A record older than this against the panel's clock says "data as of". */
const STALE_NS = 2e9;

/**
 * `nowNs` is the panel's clock (the newest trust snapshot), so "now" here is the time the header
 * shows; the vehicle's own record can be older (`d.t`), and the caption says so.
 */
export function VehicleTimeline({ d, nowNs }: { readonly d: VehicleDetail; readonly nowNs: number }): React.JSX.Element {
  const [flows, setFlows] = useState<readonly FlowRow[]>([]);
  const [err, setErr] = useState<string | null>(null);
  const set = usePkiView((s) => s.set);
  useEffect(() => {
    let live = true;
    const ask = (): void => {
      engine
        .request("inspect.flows", { device: d.device, limit: 400 }, { quiet: true, timeoutMs: 5000 })
        .then((a) => {
          if (!live) return;
          setFlows(flowsOf(a)?.instances ?? []);
          setErr(null);
        })
        .catch((e: unknown) => live && setErr(e instanceof Error ? e.message : String(e)));
    };
    ask();
    const id = window.setInterval(ask, 4000);
    return () => {
      live = false;
      window.clearInterval(id);
    };
  }, [d.device]);
  const now = Math.max(nowNs, d.t, ...flows.map((f) => f.last_t));
  const life = useMemo(() => lifecycleOf(d, flows, now), [d, flows, now]);
  const span = life.t1 - life.t0;
  const x = (t: number): number => LEFT + ((t - life.t0) / span) * (W - LEFT - PAD_R);
  const lanesH = LANES.length * LANE_H;
  const h = TOP + lanesH + AXIS_H;
  const step = tickStep(span);
  const unit = axisUnit(life.t1);
  const ticks: number[] = [];
  for (let t = Math.ceil(life.t0 / step) * step; t <= life.t1; t += step) ticks.push(t);
  const nowX = x(now);
  const stale = now - d.t > STALE_NS;
  const asOfX = x(d.t);
  return (
    <section className="pkir-life" data-testid="pki-lifecycle">
      <h4>
        Timeline{" "}
        {life.beforeRun.length > 0 ? (
          <span className="faint" data-testid="pki-life-preload" title="Enrolment and the first batches happen at the factory and dealer, before the run starts (not on the run's clock)">
            · enrolled before the run
          </span>
        ) : null}
      </h4>
      <svg viewBox={`0 0 ${W} ${h}`} width="100%" role="img" aria-label={`Certificate lifecycle of ${deviceName(d.device)}`} data-testid="pki-life-svg">
        <text x={nowX} y={10} textAnchor={nowX < W / 2 ? "start" : "end"} className="axis now-label" data-testid="pki-life-now">
          now {secondsText(now)}
        </text>
        <g transform={`translate(0 ${TOP})`}>
        {ticks.map((t) => (
          <g key={t}>
            <line x1={x(t)} x2={x(t)} y1={0} y2={lanesH} className="tick" />
            <text x={x(t)} y={lanesH + 12} textAnchor="middle" className="axis" data-testid="pki-life-tick">
              {tickLabel(t, unit)}
            </text>
          </g>
        ))}
        {LANES.map((l, k) => (
          <g key={l.id}>
            <title>{l.term}</title>
            <line x1={LEFT} x2={W - PAD_R} y1={k * LANE_H + LANE_H / 2} y2={k * LANE_H + LANE_H / 2} className="lane" />
            <text x={0} y={k * LANE_H + LANE_H / 2 + 4} className="lane-label">
              {l.label}
            </text>
          </g>
        ))}
        {life.events.map((e, k) => {
          const lane = LANES.findIndex((l) => l.id === e.lane);
          const y = lane * LANE_H + LANE_H / 2;
          const x0 = x(e.t);
          const x1 = Math.max(x0 + 3, x(e.end));
          const title = `${e.label} · ${secondsText(e.t)}${e.end > e.t ? `–${secondsText(e.end)}` : ""}${e.instance ? ` · ${e.instance}` : ""}`;
          return e.lane === "pseudonym" ? (
            <rect
              key={k}
              x={x0}
              y={y - 5}
              width={x1 - x0}
              height={10}
              className={`ev pseudo p${k % 2}`}
              tabIndex={0}
              data-lane={e.lane}
              role="button"
              onClick={() => set({ cert: { id8: e.label, holder: d.device } })}
              onKeyDown={(ev) => (ev.key === "Enter" || ev.key === " ") && set({ cert: { id8: e.label, holder: d.device } })}
            >
              <title>{`${title} (open its certificate)`}</title>
            </rect>
          ) : (
            <g key={k} className={`ev ${e.lane}`} tabIndex={0} data-lane={e.lane}>
              <title>{title}</title>
              {x1 - x0 > 6 ? <rect x={x0} y={y - 4} width={x1 - x0} height={8} rx={2} /> : null}
              <circle cx={x0} cy={y} r={4.5} />
            </g>
          );
        })}
        {stale ? (
          <line x1={asOfX} x2={asOfX} y1={0} y2={lanesH} className="asof-line">
            <title>{`The vehicle's certificate record is as of ${secondsText(d.t)}`}</title>
          </line>
        ) : null}
        <line x1={nowX} x2={nowX} y1={-4} y2={lanesH} className="now-line" />
        </g>
      </svg>
      {stale ? (
        <p
          className="faint"
          data-testid="pki-life-asof"
          title="A vehicle publishes its certificate record when its certificates change, so the pool and the pseudonym bar stop there; messages and flows run to now."
        >
          Certificate record as of {secondsText(d.t)} (dashed line).
        </p>
      ) : null}
      {err ? <p className="faint">The engine could not list this vehicle's flows: {err}</p> : null}
      <p className="faint pki-life-key">
        <span>
          <i className="k enrolment" /> enrolment
        </span>
        <span>
          <i className="k download" /> download
        </span>
        <span title="Signing with one pseudonym certificate; a new bar is a change. Click one for its certificate.">
          <i className="k pseudo" /> pseudonym in use
        </span>
        <span>
          <i className="k report" /> report sent
        </span>
        <span>
          <i className="k revocation" /> revoked
        </span>
      </p>
    </section>
  );
}
