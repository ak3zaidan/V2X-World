/**
 * The vehicle overlay: everything about the selected radio, in a dialog over the viewport.
 *
 * # Why an overlay
 *
 * This used to be a 220–360 px column beside the viewport with four tabs (Overview, Messages, Why,
 * Log). The owner's review of 2026-10-06: the column was "kind of tight on information… it can show
 * the BSMs, all the received and sent BSMs, but it's really small and compressed", the Log tab "doesn't
 * really show anything" and the Why tab "doesn't really make sense". So the column is now a large,
 * resizable dialog over the simulator, opened from the chase HUD's Details button, the header's
 * inspector button or the I key, and closed with its x or Escape. Closed, it takes nothing from the
 * viewport. It does not open by itself on selection: it would cover the vehicle being followed.
 *
 * # Sections
 *
 *  1. **Overview** — identity, health, radio, security, queues, recent trends, and the neighbours
 *     heard; compute, GNSS and clock folded away. A field with no data behind it is not drawn.
 *  2. **Sent** and **Received** — every message, one row each, click a row to read it
 *     (`components/MessagePanel.tsx`).
 *  3. **Queues** — the five queues and what is waiting in them.
 *
 * The Log tab's lines are still collected (`state.logs`, which the test handle reads) and anything a
 * user must act on is already a sentence in the status banner. The Why tab's question — which model
 * produced a number — survives as a small source note for the places that offer it (the metrics and
 * the overlay menu, `components/SourceNote.tsx`).
 */

import { Fragment, useCallback, useEffect, useMemo, useRef, useState } from "react";

import { MessageList, QueueList } from "./MessagePanel.js";
import { Sparkline } from "./Sparkline.js";
import { CloseIcon } from "../shell/Icons.js";
import { engine } from "../state/engine.js";
import { useStudio, type InspectorTab } from "../state/store.js";
import { int, radioBreakdown, radioSentence, shortDigest, simClock } from "../lib/format.js";
import { SPARKLINE_SERIES, hasData, hudGroups, queueRows, type HudField } from "../lib/telemetry.js";
import { linkText, pseudonymLine, untilText } from "../lib/security.js";
import { vehicleHealth } from "../lib/vehicleHealth.js";

import "../styles/vehicle.css";

/**
 * The overview's own names for fields whose telemetry label reads right only inside the HUD's
 * group. Everything else keeps its label, in lower case.
 */
const OVERVIEW_LABEL: Readonly<Record<string, string>> = {
  msgs_out_per_s: "sent",
  msgs_in_per_s: "heard",
  cbr_pm: "channel busy ratio",
  dcc_state: "congestion control state",
  nbr_total: "neighbours",
  nbr_verified: "verified neighbours",
  nbr_unverified: "unverified neighbours",
  nbr_revoked: "revoked neighbours",
  verifications_per_s: "verifications",
  verify_wait_p50_ms: "wait before verifying, median",
  verify_wait_p95_ms: "wait before verifying, p95",
  unverified_ratio_pm: "delivered unverified",
  crl_bytes: "CRL size",
  crl_expansion_pm: "CRL expansion done",
  full_cert_msgs: "sent with full certificate",
  p2pcd_requests: "certificate requests (P2PCD)",
};

/** A label in the overview's lower case, acronyms (CBR, TX, CPU, HSM…) left as they are. */
function overviewLabel(f: Pick<HudField, "key" | "label">): string {
  const named = OVERVIEW_LABEL[f.key];
  if (named !== undefined) return named;
  const [first = "", ...rest] = f.label.split(" ");
  const lower = /^[A-Z][a-z]/.test(first) ? first.toLowerCase() : first;
  return [lower, ...rest].join(" ");
}

/** One telemetry field as a definition-list row. */
function FieldRow({ f }: { f: HudField }): React.JSX.Element {
  const label = overviewLabel(f);
  return (
    <>
      <dt data-testid={`state-field-${f.key}`} title={f.help}>
        {label}
        {f.visibility === "GT" ? (
          <span className="gt-tag" title="Ground truth: only the simulation knows this, no radio could">
            {" "}
            GT
          </span>
        ) : null}
      </dt>
      <dd>{f.value}</dd>
    </>
  );
}

/** A plain fact (not a telemetry field): drawn only when there is a value. */
function Fact({ k, v, testId, tone }: { k: string; v: string; testId?: string; tone?: "warn" | "err" }): React.JSX.Element {
  return (
    <>
      <dt>{k}</dt>
      <dd data-testid={testId} className={tone === "warn" ? "warn-text" : tone === "err" ? "err-text" : undefined}>
        {v}
      </dd>
    </>
  );
}

/** A titled section, not drawn when every row in it is empty. */
function Section({ title, testId, children }: { title: string; testId: string; children: (React.JSX.Element | null)[] }): React.JSX.Element | null {
  if (children.every((c) => c === null)) return null;
  return (
    <div className="section vo-card" data-testid={testId}>
      <h3>{title}</h3>
      <dl className="kv">{children}</dl>
    </div>
  );
}

function EmptyState(): React.JSX.Element {
  const hello = useStudio((s) => s.hello);
  const polled = useStudio((s) => s.run.actors);
  const live = useStudio((s) => s.stats?.actorLive ?? null);
  // The node table changes on spawns and despawns; `radios` is its size and moves with it.
  const radios = useStudio((s) => s.radios);
  const breakdown = useMemo(() => {
    void radios;
    return radioBreakdown(engine.nodes.values());
  }, [radios]);
  const roadUsers = live ?? polled;

  return (
    <div className="panel-body" data-testid="inspector-empty">
      <p className="dim" data-testid="inspector-empty-message">
        {!hello
          ? "Nothing to inspect yet: no run has been loaded."
          : roadUsers > 0 || breakdown.total > 0
            ? "Select a vehicle or a roadside unit on the map to see what its radio is doing: its pseudonym, every message it sends and receives, its certificates and its queues."
            : "There is nothing on the map to select yet. Once the run has vehicles in it, choose one to see what its radio is doing."}
      </p>
      {hello ? (
        <div className="section">
          <h3>This run</h3>
          <dl className="kv">
            <dt>scenario</dt>
            <dd>{hello.scenarioName}</dd>
            <dt>on the map</dt>
            <dd data-testid="inspector-road-users">{`${int(roadUsers)} road user${roadUsers === 1 ? "" : "s"}`}</dd>
            {breakdown.total > 0 ? (
              <>
                <dt>radios</dt>
                <dd data-testid="inspector-radios">{radioSentence(breakdown)}</dd>
              </>
            ) : null}
            {hello.classNames.length > 0 ? (
              <>
                <dt>vehicle types</dt>
                <dd>{hello.classNames.join(", ")}</dd>
              </>
            ) : null}
          </dl>
        </div>
      ) : null}
    </div>
  );
}

function Overview(): React.JSX.Element {
  const telemetry = useStudio((s) => s.telemetry);
  const telemetryNode = useStudio((s) => s.telemetryNode);
  const inspect = useStudio((s) => s.inspect);
  const security = useStudio((s) => s.security);
  const pseudonym = useStudio((s) => s.pseudonym);
  const simTimeNs = useStudio((s) => s.simTimeNs);
  const selectedActor = useStudio((s) => s.selectedActor);
  const polledNeighbors = useStudio((s) => s.neighbors);
  const setTab = useStudio((s) => s.setInspectorTab);
  const devDetails = useStudio((s) => s.devDetails);
  const seriesTick = useStudio((s) => s.seriesTick);

  const groups = useMemo(() => (telemetry ? hudGroups(telemetry) : []), [telemetry]);
  const byKey = useMemo(() => new Map(groups.flatMap((g) => g.fields.map((f) => [f.key, f] as const))), [groups]);
  const queues = useMemo(() => (telemetry ? queueRows(telemetry) : []), [telemetry]);
  const health = useMemo(() => vehicleHealth(telemetry, security), [telemetry, security]);

  if (telemetryNode === null) return <EmptyState />;
  const node = telemetryNode;
  const info = engine.nodes.get(node);

  const field = (key: string): React.JSX.Element | null => {
    const f = byKey.get(key);
    return hasData(f) ? <FieldRow key={key} f={f} /> : null;
  };
  const fact = (k: string, v: string | null | undefined, testId?: string, tone?: "warn" | "err"): React.JSX.Element | null =>
    v === null || v === undefined || v === "" ? null : <Fact key={k} k={k} v={v} testId={testId} tone={tone} />;

  const neighbors = polledNeighbors ?? inspect?.neighbors ?? [];
  // The actor this radio rides on: the node table knows it for every radio the stream announced,
  // the engine's own answer and the car that was clicked are the fallbacks.
  const actorId = inspect?.actor ?? info?.actorId ?? (inspect?.kind === "rsu" ? null : selectedActor);
  const kind = inspect?.kind ?? (info?.kind === 2 ? "rsu" : info?.kind === 1 ? "vru-device" : "obu");
  const kindText = kind === "rsu" ? "roadside unit" : kind === "vru-device" ? "pedestrian or cyclist device" : kind === "obu" ? "on-board unit" : kind;
  const line = pseudonymLine(security, pseudonym);
  const pool =
    security && typeof security.pool_valid === "number"
      ? `${int(security.pool_valid)} valid${typeof security.pool_preloaded === "number" && security.pool_preloaded > 0 ? `, ${int(security.pool_preloaded)} for later periods` : ""}${typeof security.pool_stored === "number" ? `, ${int(security.pool_stored)} stored` : ""}`
      : null;
  const topup = security?.topup_in_flight === true ? "in flight" : untilText(security?.next_topup ?? null, simTimeNs);
  const crl =
    security && typeof security.crl_entries === "number"
      ? `${int(security.crl_entries)} entr${security.crl_entries === 1 ? "y" : "ies"}${typeof security.crl_version === "number" && security.crl_version > 0 ? `, version ${security.crl_version}` : ""}`
      : null;
  const reports =
    security && ((security.outbox_reports ?? 0) > 0 || (security.reports_uploaded ?? 0) > 0)
      ? `${int(security.reports_uploaded ?? 0)} uploaded, ${int(security.outbox_reports ?? 0)} waiting`
      : null;
  const shownQueues = queues.filter((q) => (q.p50 ?? 0) > 0 || (q.p95 ?? 0) > 0 || q.drops.some((d) => d.value !== null && d.value > 0));

  return (
    <div className="panel-body vo-overview" data-testid="inspector-state">
      <div className="section vo-card vo-health" data-testid="insp-health">
        <h3>Health</h3>
        {health.length === 0 ? (
          <p className="ok-text" data-testid="insp-healthy">
            Healthy: no dropped messages, the channel below the ETSI congestion threshold (CBR 0.30), valid certificates and the backend in reach.
          </p>
        ) : (
          <ul className="vo-issues">
            {health.map((h) => (
              <li key={h.key} data-testid={`insp-issue-${h.key}`}>
                <b className={h.level === "bad" ? "err-text" : "warn-text"}>{h.text}.</b> <span className="dim">{h.why}</span>
              </li>
            ))}
          </ul>
        )}
      </div>

      <Section title="Identity" testId="insp-identity">
        {[
          fact("kind", kindText),
          fact("hardware profile", inspect?.profile_id || info?.profileId || null),
          fact("radio", `node ${node}`),
          fact("road user", actorId === null || actorId === undefined ? null : `actor ${actorId}`, "inspector-actor-id"),
          fact("pseudonym", line ? shortDigest(line.digest) : null, "inspector-pseudonym"),
          fact("certificate", line?.indices ?? null, "inspector-cert-indices"),
          fact("temporary id", line?.tempId ?? null),
          fact("identity changes", typeof security?.changes === "number" && security.changes > 0 ? int(security.changes) : null),
          security?.self_revoked === true ? fact("revoked", "on the CRL: it stopped sending", "inspector-self-revoked", "err") : null,
        ]}
      </Section>

      <Section title="Radio" testId="insp-radio">
        {[
          field("msgs_out_per_s"),
          field("msgs_in_per_s"),
          field("cbr_pm"),
          field("dcc_state"),
          field("tx_power_cdbm"),
          field("airtime_ms_per_s"),
          field("nbr_total"),
          field("nbr_verified"),
          field("nbr_unverified"),
          (byKey.get("nbr_revoked")?.raw ?? 0) > 0 ? field("nbr_revoked") : null,
          (byKey.get("full_cert_msgs")?.raw ?? 0) > 0 ? field("full_cert_msgs") : null,
          (byKey.get("p2pcd_requests")?.raw ?? 0) > 0 ? field("p2pcd_requests") : null,
        ]}
      </Section>

      <Section title="Security" testId="insp-security">
        {[
          fact("certificates", pool, "inspector-pool", security?.pool_valid === 0 ? "err" : undefined),
          fact("this one expires", untilText(security?.cert_valid_until ?? null, simTimeNs)),
          fact("next top-up", topup, "inspector-topup"),
          fact("backend", security ? linkText(security) : null, undefined, security?.link_up === false ? "warn" : undefined),
          fact("revocation list", crl, "inspector-crl"),
          fact("misbehaviour reports", reports),
          field("verifications_per_s"),
          field("verify_wait_p50_ms"),
          field("verify_wait_p95_ms"),
          field("verify_policy"),
          (byKey.get("unverified_ratio_pm")?.raw ?? 0) > 0 ? field("unverified_ratio_pm") : null,
          // Without a security row, the telemetry's own counts are what there is.
          security === null ? field("cert_active") : null,
          security === null ? field("cert_stored") : null,
          security === null ? field("crl_entries") : null,
        ]}
      </Section>

      <div className="section vo-card" data-testid="insp-queues">
        <h3>Queues</h3>
        {shownQueues.length === 0 ? (
          <p className="dim">All five queues empty, nothing dropped.</p>
        ) : (
          <table className="table" data-testid="inspector-queues">
            <thead>
              <tr>
                <th>queue</th>
                <th className="num" title="Depth, median over the telemetry window">median</th>
                <th className="num" title="Depth, 95th percentile over the telemetry window">p95</th>
                <th title="Dropped in the window, by cause">dropped</th>
              </tr>
            </thead>
            <tbody>
              {shownQueues.map((q) => {
                const dropped = q.drops.filter((d) => d.value !== null && d.value > 0);
                return (
                  <tr key={q.id} data-testid={`inspector-queue-${q.id}`}>
                    <td>{q.label}</td>
                    <td className="num">{q.p50 ?? ""}</td>
                    <td className="num">{q.p95 ?? ""}</td>
                    <td className={dropped.length > 0 ? "warn-text" : undefined}>
                      {dropped.length === 0 ? "0" : dropped.map((d) => `${d.label} ${int(d.value ?? 0)}`).join(", ")}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}
        <p className="help">
          <button type="button" className="linklike" data-testid="open-queues" onClick={() => setTab("queues")}>
            What is in them now →
          </button>
        </p>
      </div>

      <div className="section vo-card vo-trends" data-testid="hud-sparklines">
        <h3>Recent trend</h3>
        <div className="sparkrow">
          {SPARKLINE_SERIES.map((s, i) => (
            <Sparkline key={s.key} seriesIndex={i} label={s.label} unit={s.unit} tick={seriesTick} />
          ))}
        </div>
      </div>

      {neighbors.length > 0 ? (
        <details className="section vo-card vo-wide insp-neighbours">
          <summary>
            <h3>Neighbours heard ({neighbors.length})</h3>
          </summary>
          <table className="table" data-testid="neighbour-table">
            <thead>
              <tr>
                <th>neighbour</th>
                <th>state</th>
                <th>messages / signal</th>
                <th>last heard</th>
              </tr>
            </thead>
            <tbody>
              {neighbors.slice(0, 40).map((n, i) => (
                // The live engine answers from its link history (node, heard/lost, RSSI); the
                // spec's row names a certificate digest and a verification state. Show either.
                <tr key={n.digest ?? `node-${n.node ?? i}`}>
                  <td>{n.digest ?? (n.node !== undefined ? `node ${n.node}` : "")}</td>
                  <td>{n.verify_state ?? n.state ?? ""}</td>
                  <td>{n.messages !== undefined ? n.messages : typeof n.rssi_dbm === "number" ? `${n.rssi_dbm.toFixed(1)} dBm` : ""}</td>
                  <td>{simClock(n.last_seen_ns)}</td>
                </tr>
              ))}
            </tbody>
          </table>
          {neighbors.length > 40 ? <p className="faint">{neighbors.length - 40} more…</p> : null}
        </details>
      ) : null}

      <MoreSection byKey={byKey} />

      {inspect?.stores && devDetails ? (
        <details className="section vo-card vo-wide">
          <summary>
            <h3>Stored on this radio (raw)</h3>
          </summary>
          <pre className="mono" style={{ fontSize: 10, whiteSpace: "pre-wrap", margin: 0 }}>
            {JSON.stringify(inspect.stores, null, 1)}
          </pre>
        </details>
      ) : null}
    </div>
  );
}

/** Compute, storage, GNSS and the clock: folded away, and only what has data. */
function MoreSection({ byKey }: { byKey: ReadonlyMap<string, HudField> }): React.JSX.Element | null {
  const keys = [
    "cpu_util_pm",
    "hsm_util_pm",
    "ram_used_kib",
    "storage_used_b",
    "node_state",
    "gnss_fix",
    "gnss_sigma_m",
    "gnss_hdop",
    "clock_drift_ppm",
    "clock_offset_ns",
    "pos_error_m",
    "peer_cache_entries",
    "crl_bytes",
    "crl_expansion_pm",
  ];
  const rows = keys.map((k) => byKey.get(k)).filter(hasData);
  if (rows.length === 0) return null;
  return (
    <details className="section vo-card vo-wide insp-more" data-testid="insp-more">
      <summary>
        <h3>Compute, positioning and clock</h3>
      </summary>
      <dl className="kv vo-kv-columns">
        {rows.map((f) => (
          <Fragment key={f.key}>
            <FieldRow f={f} />
          </Fragment>
        ))}
      </dl>
    </details>
  );
}

const TABS: readonly { id: InspectorTab; label: string; hint: string }[] = [
  { id: "overview", label: "Overview", hint: "Identity, health, radio, security and queues" },
  { id: "sent", label: "Sent", hint: "Every message this radio put on the air" },
  { id: "received", label: "Received", hint: "Every message this radio received or lost" },
  { id: "queues", label: "Queues", hint: "What is waiting inside this radio now" },
];

/** Overlay width, remembered per viewer. */
const WIDTH_KEY = "vwp.studio.vehicleOverlayWidth";
const MIN_WIDTH = 420;

function readWidth(): number | null {
  try {
    const v = Number(localStorage.getItem(WIDTH_KEY));
    return Number.isFinite(v) && v >= MIN_WIDTH ? v : null;
  } catch {
    return null;
  }
}

function writeWidth(w: number): void {
  try {
    localStorage.setItem(WIDTH_KEY, String(Math.round(w)));
  } catch {
    // A private window or blocked storage: the width is only a convenience.
  }
}

/** The dialog frame: a title with the radio's identity, the tabs, the body, and a drag handle on its left edge. */
export function Inspector({ onClose }: { onClose: () => void }): React.JSX.Element {
  const tab = useStudio((s) => s.inspectorTab);
  const setTab = useStudio((s) => s.setInspectorTab);
  const selectedNode = useStudio((s) => s.selectedNode);
  const feed = useStudio((s) => s.feed);
  const telemetry = useStudio((s) => s.telemetry);
  const security = useStudio((s) => s.security);
  const pseudonym = useStudio((s) => s.pseudonym);
  const [width, setWidth] = useState<number | null>(readWidth);
  const host = useRef<HTMLElement | null>(null);
  const health = useMemo(() => vehicleHealth(telemetry, security), [telemetry, security]);

  const info = selectedNode !== null ? engine.nodes.get(selectedNode) : undefined;
  const kind = info?.kind === 2 ? "Roadside unit" : info?.kind === 1 ? "Pedestrian device" : "Vehicle";
  const label = selectedNode !== null ? `${kind} ${info?.label || `node ${selectedNode}`}` : "Vehicle details";
  const line = pseudonymLine(security, pseudonym);

  // Focus moves into the dialog when it opens, so Escape and the tab keys work at once. Closing it
  // closes the message being read, which resumes the feed that reading paused.
  useEffect(() => {
    const el = host.current;
    if (el && !el.contains(document.activeElement)) el.focus();
    return () => {
      const open = useStudio.getState().feed.open;
      if (open) useStudio.getState().readFeedMessage(open.dir, null);
    };
  }, []);

  // A message opens inline in its own list, so leaving that list is done reading it: the message
  // closes and a pause that opening it made ends (the Queues tab would otherwise sit frozen).
  const switchTab = (t: InspectorTab): void => {
    const open = useStudio.getState().feed.open;
    if (open && open.dir !== t) useStudio.getState().readFeedMessage(open.dir, null);
    setTab(t);
  };

  const onDrag = useCallback((ev: React.PointerEvent<HTMLDivElement>) => {
    const el = host.current;
    const stage = el?.parentElement;
    if (!el || !stage) return;
    ev.preventDefault();
    const right = el.getBoundingClientRect().right;
    const max = stage.getBoundingClientRect().width - 16;
    const move = (e: PointerEvent): void => {
      const w = Math.min(max, Math.max(MIN_WIDTH, right - e.clientX));
      setWidth(w);
    };
    const up = (e: PointerEvent): void => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      writeWidth(Math.min(max, Math.max(MIN_WIDTH, right - e.clientX)));
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }, []);

  const count = (t: InspectorTab): string | null =>
    selectedNode === null ? null : t === "sent" ? String(feed.sent.length) : t === "received" ? String(feed.received.length) : null;

  return (
    <section
      ref={host}
      className="vehicle-overlay"
      role="dialog"
      aria-label={label}
      tabIndex={-1}
      data-testid="inspector-panel"
      style={width !== null ? { width } : undefined}
    >
      <div
        className="vo-resize"
        onPointerDown={onDrag}
        onDoubleClick={() => {
          setWidth(null);
          writeWidth(0);
        }}
        role="separator"
        aria-orientation="vertical"
        aria-label="Drag to resize; double-click to reset"
        title="Drag to resize; double-click to reset"
      />
      <header className="vo-head">
        <div className="vo-title">
          <h2 data-testid="inspector-title">{label}</h2>
          {line ? (
            <span className="dim mono" title={line.digest}>
              pseudonym {shortDigest(line.digest)}
              {line.indices ? ` (${line.indices})` : ""}
            </span>
          ) : null}
          {selectedNode !== null && telemetry ? (
            health.length === 0 ? (
              <span className="vo-chip ok" data-testid="inspector-health">
                healthy
              </span>
            ) : (
              <span className={`vo-chip ${health[0].level}`} data-testid="inspector-health" title={health.map((h) => h.text).join("\n")}>
                {health.length === 1 ? health[0].text : `${health.length} issues`}
              </span>
            )
          ) : null}
        </div>
        <button type="button" className="icon-button" onClick={onClose} aria-label="Close the vehicle details" title="Close (Esc)" data-testid="inspector-close">
          <CloseIcon />
        </button>
      </header>
      <div className="vo-tabs tabs" role="tablist" aria-label="Vehicle details">
        {TABS.map((t) => {
          const n = count(t.id);
          return (
            <button
              key={t.id}
              type="button"
              role="tab"
              aria-selected={tab === t.id}
              className={tab === t.id ? "active" : ""}
              onClick={() => switchTab(t.id)}
              data-testid={`tab-${t.id}`}
              title={t.hint}
            >
              {t.label}
              {n !== null ? <span className="count">{n}</span> : null}
            </button>
          );
        })}
      </div>
      <div className="vo-body">
        {tab === "overview" ? <Overview /> : null}
        {tab === "sent" ? <MessageList dir="sent" /> : null}
        {tab === "received" ? <MessageList dir="received" /> : null}
        {tab === "queues" ? <QueueList /> : null}
      </div>
    </section>
  );
}
