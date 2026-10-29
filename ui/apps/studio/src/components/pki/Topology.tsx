/**
 * The live topology of the SCMS deployment: the SCMS Manager and the electors on top, each
 * provider as a region with its authorities in their places and a ghost where it runs no
 * component of a role, its vehicles and roadside units as one client group along the bottom,
 * the links the architecture defines between them, and every message of the run drawn as a dot
 * travelling its link as `inspect.trust` reports it (`lib/pkiTopology.ts` lays it out).
 *
 * Nodes are buttons (focus, Enter, a tooltip with the standard term and what the role does).
 * Message dots are CSS `offset-path` animations, staggered over the poll interval by their
 * simulated time; with reduced motion they are not drawn and the links only light.
 */

import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";

import { GROUND_TRUTH, formatSpan, roleLabel, type PkiComponent, type TrustAnswer } from "../../lib/pki.js";
import {
  FLOW_INFO,
  flowInfo,
  layoutTopology,
  linkKey,
  maModeText,
  linkLoad,
  edgePath,
  messageEnds,
  messageText,
  messagesOf,
  observedBy,
  roleInfo,
  type TopoNode,
  type TrustMessage,
} from "../../lib/pkiTopology.js";

/** Wall-clock ms a message dot takes along its link. */
const TRAVEL_MS = 1100;
/** Dots drawn per poll at most (the rest only light their links). */
const MAX_DOTS = 60;


export function LockIcon(): React.JSX.Element {
  return (
    <svg className="pki-lock" viewBox="0 0 12 12" width="10" height="10" aria-hidden="true">
      <rect x="2" y="5.5" width="8" height="5.5" rx="1" fill="currentColor" />
      <path d="M4 5.5V4a2 2 0 0 1 4 0v1.5" fill="none" stroke="currentColor" strokeWidth="1.3" />
    </svg>
  );
}

interface Dot {
  readonly key: string;
  /** The link's two ends, sender first: the path is read from the layout drawn now, never an old one. */
  readonly a: string;
  readonly b: string;
  readonly color: string;
  readonly delay: number;
  readonly title: string;
}

function useWidth(): [React.RefObject<HTMLDivElement | null>, number] {
  const ref = useRef<HTMLDivElement | null>(null);
  const [w, setW] = useState(1100);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return undefined;
    const measure = (): void => setW(Math.max(320, Math.floor(el.clientWidth)));
    measure();
    if (typeof ResizeObserver === "undefined") return undefined;
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  return [ref, w];
}

function reducedMotion(): boolean {
  try {
    return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  } catch {
    return false;
  }
}

export function Topology({
  trust,
  messages,
  view,
  selected,
  onSelect,
}: {
  readonly trust: TrustAnswer;
  /** The messages of the newest answer not drawn before. */
  readonly messages: readonly TrustMessage[];
  readonly view: string;
  readonly selected: string | null;
  readonly onSelect: (id: string) => void;
}): React.JSX.Element {
  const [ref, width] = useWidth();
  const s = trust.state;
  const all = useMemo(() => messagesOf(trust), [trust]);
  const topo = useMemo(() => layoutTopology(s, width, all), [s, width, all]);
  const byId = useMemo(() => new Map(topo.nodes.map((n) => [n.id, n])), [topo]);
  const seen = all.filter((m) => observedBy(m, view));
  const load = useMemo(() => linkLoad(seen), [seen]);
  const [dots, setDots] = useState<Dot[]>([]);

  // New messages become dots, staggered by their simulated time over the poll interval.
  useEffect(() => {
    const fresh = messages.filter((m) => observedBy(m, view));
    if (fresh.length === 0 || reducedMotion()) return;
    const t0 = fresh[0].t;
    const span = Math.max(1, fresh[fresh.length - 1].t - t0);
    const next: Dot[] = [];
    for (const m of fresh.slice(-MAX_DOTS)) {
      const ends = messageEnds(topo, m);
      if (!ends) continue;
      next.push({
        key: `${m.id}`,
        a: ends[0],
        b: ends[1],
        color: flowInfo(m.flow).color,
        delay: Math.round(((m.t - t0) / span) * 700),
        title: `${messageText(m.name)} · ${m.from ?? "?"} → ${m.to ?? "?"}`,
      });
    }
    setDots((old) => [...old.slice(-MAX_DOTS), ...next]);
    // `topo` is read at the instant the messages arrive; a relayout does not re-send them.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [messages, view]);

  const done = (key: string): void => setDots((old) => old.filter((d) => d.key !== key));
  const drawn = new Set(topo.links.map((l) => l.key));
  const signers = new Set(s.ctl.signers);

  /** One node, positioned in its container (a region at `ox`, `oy`, or the governance strip). */
  const renderNode = (n: TopoNode, ox: number, oy: number): React.JSX.Element => {
    const at = { ...n, x: n.x - ox, y: n.y - oy };
    if (n.kind === "devices") return <DevicesNode key={n.id} n={at} trust={trust} selected={selected === n.id} onSelect={onSelect} />;
    if (n.kind === "ghost")
      return (
        <div
          key={n.id}
          className="pki-node ghost"
          style={{ left: at.x, top: at.y, width: n.w, height: n.h }}
          tabIndex={0}
          title={`${roleInfo(n.role).term}: not deployed by ${n.provider}`}
          data-testid={`pki-ghost-${n.provider}/${n.role}`}
          data-status="not-deployed"
        >
          {roleInfo(n.role).name}
          <small className="acr">not deployed</small>
        </div>
      );
    const c = n.component as PkiComponent;
    return (
      <NodeButton
        key={n.id}
        n={at}
        c={c}
        busy={(trust.activity[n.id]?.messages ?? 0) > 0}
        selected={selected === n.id}
        viewing={view !== GROUND_TRUTH && view === n.id}
        signed={n.role === "elector" ? signers.has(c.instance) : null}
        onSelect={onSelect}
      />
    );
  };

  return (
    <div className="pki-topo-wrap" ref={ref} data-testid="pki-topology">
      <TopoLegend />
      <div className="pki-topo" style={{ width: topo.width, height: topo.height }} data-layout={topo.sideBySide ? "side-by-side" : "stacked"}>
        <div className="pki-topo-gov" style={{ left: 0, top: 0, width: topo.governance.w, height: topo.governance.h }} data-testid="pki-governance" aria-label="Governance">
          {topo.nodes.filter((n) => !topo.regions.some((r) => r.provider.label === n.provider)).map((n) => renderNode(n, 0, 0))}
        </div>
        {topo.regions.map((r) => (
          <section
            key={r.provider.label}
            className={r.provider.on_ctl ? "pki-region" : "pki-region off-ctl"}
            style={{ left: r.x, top: r.y, width: r.w, height: r.h }}
            data-testid={`pki-provider-${r.provider.label}`}
            aria-label={`${r.provider.label}`}
          >
            <header>
              <b className="mono">{r.provider.label}</b>
              {r.provider.home ? (
                <span className="pill" title="The home provider: the one this scenario's own vehicles enrol with first; the other providers' fleets roam in">
                  home
                </span>
              ) : null}
              <span className={r.provider.on_ctl ? "pill ok" : "pill warn"} title={r.provider.on_ctl ? "Its root is on the certificate trust list (CTL): devices accept its certificates" : "Its root is not on the trust list: devices reject its certificates"}>
                {r.provider.on_ctl ? "trusted" : "not trusted"}
              </span>
              <span className="dim" data-testid={`pki-region-${r.provider.label}`} title={r.provider.region.assumption ? "No provider publishes where it runs: this region is an assumption (01 §2.3)" : undefined}>
                {r.provider.region.metro}
                {r.provider.region.assumption ? <em className="assume"> assumed</em> : null}
              </span>
              <span className="dim" title={`How its Misbehaviour Authority (MA) decides a case${r.provider.ma_review_s !== null ? `; an operator reviews each case for ${formatSpan(r.provider.ma_review_s * 1e9)}` : ""}`}>
                {maModeText(r.provider.ma_mode)}
              </span>
            </header>
            {topo.nodes.filter((n) => n.provider === r.provider.label).map((n) => renderNode(n, r.x, r.y))}
          </section>
        ))}
        <svg className="pki-links" width={topo.width} height={topo.height} aria-hidden="true">
          {topo.links.map((l) => {
            const a = byId.get(l.a);
            const b = byId.get(l.b);
            if (!a || !b) return null;
            const n = load.get(linkKey(l.a, l.b)) ?? 0;
            const off = a.kind === "ghost" || b.kind === "ghost" || a.component?.status === "offline" || b.component?.status === "offline";
            return (
              <path
                key={l.key}
                d={edgePath(topo, a, b)}
                className={`pki-link${n > 0 ? " busy" : ""}${l.defined ? "" : " undeclared"}${off ? " off" : ""}`}
                // The bold stroke is kept for live traffic on the architecture's own links; a link it
                // does not draw stays thin and grey however busy, so the eye goes to the regular paths.
                style={n > 0 && l.defined ? { strokeWidth: Math.min(5, 1.5 + Math.log2(1 + n)) } : undefined}
                data-link={l.key}
                data-load={n}
              />
            );
          })}
        </svg>
        <div className="pki-dots" aria-hidden="true">
          {dots.map((d) => {
            // Only along a link the diagram draws now, in the layout drawn now.
            const a = byId.get(d.a);
            const b = byId.get(d.b);
            if (!a || !b || !drawn.has(linkKey(d.a, d.b))) return null;
            return (
              <i
                key={d.key}
                className="pki-msg-dot"
                style={{ offsetPath: `path('${edgePath(topo, a, b)}')`, background: d.color, animationDelay: `${d.delay}ms`, animationDuration: `${TRAVEL_MS}ms` }}
                onAnimationEnd={() => done(d.key)}
                title={d.title}
              />
            );
          })}
        </div>
      </div>
    </div>
  );
}

/** What the diagram's colours and marks mean, above it so it is read first. */
function TopoLegend(): React.JSX.Element {
  return (
    <p className="pki-topo-legend dim" data-testid="pki-topo-legend">
      <span className="faint">Messages:</span>
      {Object.entries(FLOW_INFO).map(([k, f]) => (
        <span key={k} className="pki-key">
          <i className="pki-swatch" style={{ background: f.color }} /> {f.name}
        </span>
      ))}
      <span className="faint pki-key-sep">Boxes:</span>
      <span className="pki-key busy" title="The dot in a box's corner lights green while it sends or receives">
        <i className="pki-dot" /> busy in the last 2 s
      </span>
      <span className="pki-key" title="A grey corner dot: online, nothing in the last 2 s">
        <i className="pki-dot idle" /> idle
      </span>
      <span className="pki-key">
        <LockIcon /> offline by design
      </span>
      <span className="pki-key">
        <i className="pki-swatch ghost" /> not deployed
      </span>
      <span className="pki-key" title="A solid line is a path the US SCMS architecture defines; it turns blue and thick while messages travel it">
        <i className="pki-link-key solid" /> link, <i className="pki-link-key busy" /> busy
      </span>
      <span
        className="pki-key"
        title="Messages this run sent along a path the base architecture does not draw: for example two providers' Misbehaviour Authorities sharing reports, which this scenario turns on"
      >
        <i className="pki-link-key" /> extra link this scenario adds
      </span>
      <span className="pki-key faulted" title="A component with an injected outage: red border, red corner dot">
        <i className="pki-swatch outage" /> outage
      </span>
    </p>
  );
}

function NodeButton({
  n,
  c,
  busy,
  selected,
  viewing,
  signed,
  onSelect,
}: {
  readonly n: TopoNode;
  readonly c: PkiComponent;
  readonly busy: boolean;
  readonly selected: boolean;
  readonly viewing: boolean;
  readonly signed: boolean | null;
  readonly onSelect: (id: string) => void;
}): React.JSX.Element {
  const info = roleInfo(c.role);
  const cls = ["pki-node", "pki-comp", `status-${c.status}`, `role-${c.role}`];
  if (n.w < 100) cls.push("narrow");
  if (busy) cls.push("busy");
  if (c.queue && c.queue.depth > 0) cls.push("queued");
  if (selected) cls.push("selected");
  if (viewing) cls.push("viewing");
  if (signed === false) cls.push("unsigned");
  const status = c.status === "offline" ? "offline by design" : c.status === "faulted" ? "outage" : "online";
  return (
    <button
      type="button"
      className={cls.join(" ")}
      style={{ left: n.x, top: n.y, width: n.w, height: n.h }}
      onClick={() => onSelect(c.id)}
      title={`${info.term}${c.instance > 0 ? ` #${c.instance + 1}` : ""} — ${status}${c.queue && c.queue.depth > 0 ? ` · ${c.queue.depth} queued` : ""}\n${info.does}`}
      aria-label={`${info.name}${c.instance > 0 ? ` ${c.instance + 1}` : ""}, ${c.provider}, ${status}`}
      aria-pressed={selected}
      data-testid={`pki-comp-${c.id}`}
      data-status={c.status}
    >
      {c.status === "offline" ? <LockIcon /> : <i className="pki-dot" aria-hidden="true" />}
      <span className="r">
        <span className="nm">{c.role === "elector" || c.instance > 0 ? `${info.name} ${c.instance + 1}` : info.name}</span>
        {c.role !== "elector" && c.role !== "manager" && roleLabel(c.role) !== info.name ? <small className="acr">{roleLabel(c.role)}</small> : null}
      </span>
      {c.queue && c.queue.depth > 0 ? <span className="pki-q">{c.queue.depth}</span> : null}
    </button>
  );
}

function DevicesNode({
  n,
  trust,
  selected,
  onSelect,
}: {
  readonly n: TopoNode;
  readonly trust: TrustAnswer;
  readonly selected: boolean;
  readonly onSelect: (id: string) => void;
}): React.JSX.Element {
  const p = trust.state.providers.find((x) => x.label === n.provider);
  const d = p?.devices;
  const info = roleInfo("devices");
  return (
    <button
      type="button"
      className={selected ? "pki-node devices selected" : "pki-node devices"}
      style={{ left: n.x, top: n.y, width: n.w, height: n.h }}
      onClick={() => onSelect(n.id)}
      title={`${info.term} of ${n.provider}\n${info.does}`}
      data-testid={`pki-devices-${n.provider}`}
    >
      <span className="r">{info.name}</span>
      {d ? (
        <span className="dim pki-devices-line">
          <b>{d.devices}</b> devices · {d.enrolled} enrolled · {d.provisioned} with pseudonym certificates
          {d.blocklisted > 0 ? (
            <span title="Refused by the RA after a revocation: their enrolment certificate is on its blocklist. How many devices are revoked is in the strip above."> · {d.blocklisted} blocked from new certificates</span>
          ) : (
            ""
          )}
        </span>
      ) : null}
    </button>
  );
}
