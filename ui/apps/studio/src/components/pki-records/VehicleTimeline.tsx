/**
 * A vehicle's certificate lifecycle as a timeline (`lib/pkiVehicle.ts`): enrolment, the pseudonym
 * downloads and top-ups, the pseudonym it signs with and each change, the reports it sent and its
 * revocation, on the run's clock, with "now" marked and a tick every round interval; the factory
 * and dealer steps before the run are listed apart. Each event is focusable and names its flow
 * instance (the Messages section shows it whole); a pseudonym's bar opens its certificate.
 */

import { useEffect, useMemo, useState } from "react";

import { flowsOf, type FlowRow } from "../../lib/pki.js";
import { secondsText, type VehicleDetail } from "../../lib/pkiRecords.js";
import { LANES, lifecycleOf, tickStep } from "../../lib/pkiVehicle.js";
import { engine } from "../../state/engine.js";
import { usePkiView } from "./state.js";

const W = 640;
const LANE_H = 22;
const LEFT = 92;
const PAD_R = 10;
const AXIS_H = 20;

function tickText(ns: number): string {
  const s = Math.round(ns / 1e9);
  if (s < 120) return `${s} s`;
  if (s < 7200) return `${Math.round(s / 60)} min`;
  if (s < 172_800) return `${Math.round(s / 3600)} h`;
  return `${Math.round(s / 86_400)} d`;
}

export function VehicleTimeline({ d }: { readonly d: VehicleDetail }): React.JSX.Element {
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
  const life = useMemo(() => lifecycleOf(d, flows, d.t), [d, flows]);
  const span = life.t1 - life.t0;
  const x = (t: number): number => LEFT + ((t - life.t0) / span) * (W - LEFT - PAD_R);
  const h = LANES.length * LANE_H + AXIS_H;
  const step = tickStep(span);
  const ticks: number[] = [];
  for (let t = Math.ceil(life.t0 / step) * step; t <= life.t1; t += step) ticks.push(t);
  const nowX = x(d.t);
  return (
    <section className="pkir-life" data-testid="pki-lifecycle">
      <h4>Certificate life on the run's clock</h4>
      {life.beforeRun.length > 0 ? (
        <p className="faint" data-testid="pki-life-preload" title="Enrolment and the first batches happen at the factory and dealer, before the run starts (not on the run's clock)">
          Before the run: enrolled and given its first batches at the factory and dealer ({life.beforeRun.length} step{life.beforeRun.length === 1 ? "" : "s"}).
        </p>
      ) : null}
      <svg viewBox={`0 0 ${W} ${h}`} width="100%" role="img" aria-label={`Certificate lifecycle of ${d.device}`}>
        {ticks.map((t) => (
          <g key={t}>
            <line x1={x(t)} x2={x(t)} y1={0} y2={LANES.length * LANE_H} className="tick" />
            <text x={x(t)} y={LANES.length * LANE_H + 12} textAnchor="middle" className="axis">
              {tickText(t)}
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
        <line x1={nowX} x2={nowX} y1={0} y2={LANES.length * LANE_H} className="now-line" />
        <text x={nowX} y={h - 1} textAnchor={nowX < W / 2 ? "start" : "end"} className="axis now-label">
          now {secondsText(d.t)}
        </text>
        {nowX - LEFT > 90 ? (
          <text x={LEFT} y={h - 1} className="axis">
            {secondsText(life.t0)}
          </text>
        ) : null}
      </svg>
      {err ? <p className="faint">The engine could not list this vehicle's flows: {err}</p> : null}
      <p className="faint pki-life-key">
        <span>
          <i className="k enrolment" /> enrolment
        </span>
        <span>
          <i className="k download" /> certificate download (first batch or top-up)
        </span>
        <span>
          <i className="k pseudo" /> signing with one pseudonym (a new bar is a change; click it)
        </span>
        <span>
          <i className="k report" /> misbehaviour report sent
        </span>
        <span>
          <i className="k revocation" /> revoked
        </span>
      </p>
      <p className="faint mono" data-testid="pki-life-batches" title="Certificates it holds per week (i-period): 20 a week in the US profile">
        held per week: {life.batches.map((b) => `i${b.i} ${b.certs}`).join(" · ") || "none"}
      </p>
    </section>
  );
}
