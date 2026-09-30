/**
 * The right-hand inspector: what one radio is doing, why a number is what it is, and the log.
 *
 * # The overview
 *
 * Laid out around what a researcher looks at when they select a vehicle, in this order:
 *
 *  1. **Identity** — what it is, its hardware profile, and the pseudonym it is signing with now, with
 *     the certificate's i-period and j index and the temporary id its messages carry.
 *  2. **Radio** — what it sends and hears per second, the channel load and the congestion state it
 *     is under, its power, and its neighbours.
 *  3. **Security** — the certificate pool, when it next tops up, the backend it reaches, the CRL it
 *     holds, and how fast it verifies.
 *  4. **Queues** — its five queues' depths and what each dropped, and a door to the live queue
 *     contents under Messages.
 *  5. **More** — compute, storage, GNSS and the clock, folded away.
 *
 * A field with no data behind it — a value at its "not modelled" sentinel, a security row in a run
 * with no credential system — is not drawn. It used to be drawn as "n/a", which on most runs was a
 * third of the panel, and a JSON dump of the node's stores sat under it. A section with nothing in
 * it is not drawn either.
 *
 * # The empty state
 *
 * It says what is on the map, with each number named for what it counts: road users (every vehicle,
 * pedestrian and cyclist) and radios (on vehicles, carried, roadside), so "82 radios" beside "106
 * road users" is two true statements and not a contradiction (QA, 2026-09-24).
 */

import { Fragment, useMemo } from "react";

import { MessagePanel } from "./MessagePanel.js";
import { ObuHud } from "./ObuHud.js";
import { WhyTab } from "./WhyTab.js";
import { engine } from "../state/engine.js";
import { useStudio } from "../state/store.js";
import { int, radioBreakdown, radioSentence, simClock } from "../lib/format.js";
import { hasData, hudGroups, queueRows, type HudField } from "../lib/telemetry.js";
import { linkText, pseudonymLine, untilText } from "../lib/security.js";

/**
 * The overview's own names for fields whose telemetry label only reads right inside the HUD's
 * group: "verified" under a "neighbours" heading is a count of neighbours, but in the overview's
 * Radio section, beside "tx" and "CBR", it has to say so. Everything else keeps its label, in the
 * section's lower case.
 */
const OVERVIEW_LABEL: Readonly<Record<string, string>> = {
  nbr_total: "neighbours",
  nbr_verified: "verified neighbours",
  nbr_unverified: "unverified neighbours",
  nbr_revoked: "revoked neighbours",
  verifications_per_s: "verifications",
  crl_bytes: "CRL size",
};

/** A label in the overview's lower case, acronyms (CBR, TX, CPU, HSM…) left as they are. */
function overviewLabel(f: Pick<HudField, "key" | "label">): string {
  const named = OVERVIEW_LABEL[f.key];
  if (named !== undefined) return named;
  const [first = "", ...rest] = f.label.split(" ");
  const lower = /^[A-Z][a-z]/.test(first) ? first.toLowerCase() : first;
  return [lower, ...rest].join(" ");
}

/** One telemetry field as a definition-list row whose name opens its provenance. */
function FieldRow({ f: raw, node }: { f: HudField; node: number }): React.JSX.Element {
  const setWhy = useStudio((s) => s.setWhy);
  const f = { ...raw, label: overviewLabel(raw) };
  return (
    <>
      <dt>
        <button
          type="button"
          className="linklike"
          data-testid={`state-field-${f.key}`}
          aria-label={`${f.label}: ${f.value} — explain`}
          aria-describedby={`state-help-${f.key}`}
          onClick={() => setWhy({ kind: "node_field", id: f.key, label: f.label, node, value: f.value, unit: f.unit })}
        >
          {f.label}
          {f.visibility === "GT" ? <span className="gt-tag"> GT</span> : null}
          <span className="sr-only" id={`state-help-${f.key}`}>
            {f.key} · {f.unit}
            {f.help ? ` — ${f.help}` : ""}
          </span>
        </button>
      </dt>
      <dd>{f.value}</dd>
    </>
  );
}

/** A plain fact (not a telemetry field): drawn only when there is a value. */
function Fact({ k, v, testId }: { k: string; v: string | null | undefined; testId?: string }): React.JSX.Element | null {
  if (v === null || v === undefined || v === "") return null;
  return (
    <>
      <dt>{k}</dt>
      <dd data-testid={testId}>{v}</dd>
    </>
  );
}

/** A titled section, not drawn when every row in it is empty. */
function Section({ title, testId, children }: { title: string; testId: string; children: (React.JSX.Element | null)[] }): React.JSX.Element | null {
  if (children.every((c) => c === null)) return null;
  return (
    <div className="section insp-section" data-testid={testId}>
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
          ? "Nothing to inspect yet — no run has been loaded."
          : roadUsers > 0 || breakdown.total > 0
            ? "Select a vehicle or a roadside unit on the map and what its radio is doing appears here: its pseudonym, what it sends and hears, its certificates and its queues."
            : "There is nothing on the map to select yet. Once the run has vehicles in it, choose one and what its radio is doing appears here."}
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

function StateTab(): React.JSX.Element {
  const telemetry = useStudio((s) => s.telemetry);
  const telemetryNode = useStudio((s) => s.telemetryNode);
  const inspect = useStudio((s) => s.inspect);
  const security = useStudio((s) => s.security);
  const pseudonym = useStudio((s) => s.pseudonym);
  const simTimeNs = useStudio((s) => s.simTimeNs);
  const selectedActor = useStudio((s) => s.selectedActor);
  const polledNeighbors = useStudio((s) => s.neighbors);
  const setTab = useStudio((s) => s.setInspectorTab);
  const setFeedTab = useStudio((s) => s.setFeedTab);
  const devDetails = useStudio((s) => s.devDetails);

  const groups = useMemo(() => (telemetry ? hudGroups(telemetry) : []), [telemetry]);
  const byKey = useMemo(() => new Map(groups.flatMap((g) => g.fields.map((f) => [f.key, f] as const))), [groups]);
  const queues = useMemo(() => (telemetry ? queueRows(telemetry) : []), [telemetry]);

  if (telemetryNode === null) return <EmptyState />;
  const node = telemetryNode;
  const info = engine.nodes.get(node);

  const field = (key: string): React.JSX.Element | null => {
    const f = byKey.get(key);
    return hasData(f) ? <FieldRow key={key} f={f} node={node} /> : null;
  };
  const fact = (k: string, v: string | null | undefined, testId?: string): React.JSX.Element | null =>
    v === null || v === undefined || v === "" ? null : <Fact key={k} k={k} v={v} testId={testId} />;

  const neighbors = polledNeighbors ?? inspect?.neighbors ?? [];
  // The actor this radio rides on: the node table knows it for every radio the stream announced,
  // the engine's own answer and the car that was clicked are the fallbacks.
  const actorId = inspect?.actor ?? info?.actorId ?? (inspect?.kind === "rsu" ? null : selectedActor);
  const kind = inspect?.kind ?? (info?.kind === 2 ? "rsu" : info?.kind === 1 ? "vru-device" : "obu");
  const kindText = kind === "rsu" ? "roadside unit" : kind === "vru-device" ? "pedestrian or cyclist device" : kind === "obu" ? "on-board unit" : kind;
  const line = pseudonymLine(security, pseudonym);
  const pool =
    security && typeof security.pool_valid === "number"
      ? `${int(security.pool_valid)} valid${typeof security.pool_preloaded === "number" ? `, ${int(security.pool_preloaded)} for later periods` : ""}${typeof security.pool_stored === "number" ? `, ${int(security.pool_stored)} stored` : ""}`
      : null;
  const topup = security?.topup_in_flight === true ? "in flight" : untilText(security?.next_topup ?? null, simTimeNs);
  const crl =
    security && typeof security.crl_entries === "number"
      ? `${int(security.crl_entries)} entr${security.crl_entries === 1 ? "y" : "ies"}${typeof security.crl_version === "number" && security.crl_version > 0 ? `, version ${security.crl_version}` : ""}`
      : null;
  const reports =
    security && (typeof security.outbox_reports === "number" || typeof security.reports_uploaded === "number")
      ? `${int(security.reports_uploaded ?? 0)} uploaded, ${int(security.outbox_reports ?? 0)} waiting`
      : null;
  const shownQueues = queues.filter((q) => q.p50 !== null || q.p95 !== null || q.drops.some((d) => d.value !== null && d.value > 0));

  return (
    <div className="panel-body" data-testid="inspector-state">
      <Section title="Identity" testId="insp-identity">
        {[
          fact("kind", kindText),
          fact("profile", inspect?.profile_id || info?.profileId || null),
          fact("node", String(node)),
          fact("actor", actorId === null || actorId === undefined ? null : String(actorId), "inspector-actor-id"),
          fact("pseudonym", line ? `${line.digest.slice(0, 16)}${line.digest.length > 16 ? "…" : ""}` : null, "inspector-pseudonym"),
          fact("certificate", line?.indices ?? null, "inspector-cert-indices"),
          fact("temporary id", line?.tempId ?? null),
          fact("changes so far", typeof security?.changes === "number" ? int(security.changes) : null),
          security?.self_revoked === true ? fact("revoked", "on the CRL: it stopped sending", "inspector-self-revoked") : null,
          devDetails && inspect ? fact("sampled at", simClock(inspect.t_ns)) : null,
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
          field("full_cert_msgs"),
          field("p2pcd_requests"),
          field("nbr_total"),
          field("nbr_verified"),
          field("nbr_unverified"),
          field("nbr_revoked"),
        ]}
      </Section>

      {neighbors.length > 0 ? (
        <details className="section insp-neighbours">
          <summary>
            <h3>Neighbours heard ({neighbors.length})</h3>
          </summary>
          <table className="table" data-testid="neighbour-table">
            <thead>
              <tr>
                <th>neighbour</th>
                <th>state</th>
                <th>msgs / RSSI</th>
                <th>last seen</th>
              </tr>
            </thead>
            <tbody>
              {neighbors.slice(0, 20).map((n, i) => (
                // The live engine answers from its link history (node, heard/lost, RSSI); the
                // spec's row names a certificate digest and a verification state. Show either.
                <tr key={n.digest ?? `node-${n.node ?? i}`}>
                  <td>{n.digest ?? (n.node !== undefined ? `node ${n.node}` : "")}</td>
                  <td>{n.verify_state ?? n.state ?? ""}</td>
                  <td>
                    {n.messages !== undefined ? n.messages : typeof n.rssi_dbm === "number" ? `${n.rssi_dbm.toFixed(1)} dBm` : ""}
                  </td>
                  <td>{simClock(n.last_seen_ns)}</td>
                </tr>
              ))}
            </tbody>
          </table>
          {neighbors.length > 20 ? <p className="faint">{neighbors.length - 20} more…</p> : null}
        </details>
      ) : null}

      <Section title="Security" testId="insp-security">
        {[
          fact("certificates", pool, "inspector-pool"),
          fact("this one expires", untilText(security?.cert_valid_until ?? null, simTimeNs)),
          fact("next top-up", topup, "inspector-topup"),
          fact("backend", security ? linkText(security) : null),
          fact("CRL", crl, "inspector-crl"),
          field("crl_bytes"),
          field("crl_expansion_pm"),
          fact("misbehaviour reports", reports),
          field("verifications_per_s"),
          field("verify_wait_p50_ms"),
          field("verify_wait_p95_ms"),
          field("verify_policy"),
          field("unverified_ratio_pm"),
          field("peer_cache_entries"),
          // Without a security row, the telemetry's own counts are what there is.
          security === null ? field("cert_active") : null,
          security === null ? field("cert_stored") : null,
          security === null ? field("crl_entries") : null,
        ]}
      </Section>

      {shownQueues.length > 0 ? (
        <div className="section insp-section" data-testid="insp-queues">
          <h3>Queues</h3>
          <table className="table" data-testid="inspector-queues">
            <thead>
              <tr>
                <th>queue</th>
                <th title="depth, median over the telemetry window">p50</th>
                <th title="depth, 95th percentile over the telemetry window">p95</th>
                <th title="dropped in the window, by cause">dropped</th>
              </tr>
            </thead>
            <tbody>
              {shownQueues.map((q) => {
                const dropped = q.drops.filter((d) => d.value !== null && d.value > 0);
                return (
                  <tr key={q.id} data-testid={`inspector-queue-${q.id}`}>
                    <td>{q.label}</td>
                    <td>{q.p50 ?? ""}</td>
                    <td>{q.p95 ?? ""}</td>
                    <td className={dropped.length > 0 ? "warn-text" : undefined}>
                      {dropped.length === 0 ? "0" : dropped.map((d) => `${d.label} ${int(d.value ?? 0)}`).join(", ")}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
          <p className="help">
            What is in them now, message by message:{" "}
            <button
              type="button"
              className="linklike"
              data-testid="open-queues"
              onClick={() => {
                setFeedTab("queues");
                setTab("messages");
              }}
            >
              Messages → Queues
            </button>
            .
          </p>
        </div>
      ) : null}

      <MoreSection byKey={byKey} node={node} />

      {inspect?.stores && devDetails ? (
        <details className="section">
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
function MoreSection({ byKey, node }: { byKey: ReadonlyMap<string, HudField>; node: number }): React.JSX.Element | null {
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
    "outbox_bytes",
  ];
  const rows = keys.map((k) => byKey.get(k)).filter(hasData);
  if (rows.length === 0) return null;
  return (
    <details className="section insp-more" data-testid="insp-more">
      <summary>
        <h3>Compute, GNSS and clock</h3>
      </summary>
      <dl className="kv">
        {rows.map((f) => (
          <Fragment key={f.key}>
            <FieldRow f={f} node={node} />
          </Fragment>
        ))}
      </dl>
    </details>
  );
}

function LogTab(): React.JSX.Element {
  const logs = useStudio((s) => s.logs);
  return (
    <div className="panel-body log" data-testid="inspector-log">
      {logs.length === 0 ? (
        <p className="dim">
          Nothing to report. Anything the engine or this page has to say about a run — a refused frame, a
          failed command, a world that did not match — appears here as it happens.
        </p>
      ) : null}
      {logs.map((l, i) => (
        <div className="line" key={`${l.at}-${i}`}>
          <span className={`lvl ${l.level}`}>{l.level}</span>
          <span className="tgt">{l.target}</span>
          <span className="grow">{l.message}</span>
        </div>
      ))}
    </div>
  );
}

const TABS: readonly { id: "state" | "messages" | "why" | "log"; label: string; hint: string }[] = [
  { id: "state", label: "Overview", hint: "The selected radio: identity, radio, security and queues" },
  { id: "messages", label: "Messages", hint: "What the followed radio sends and hears, message by message, and its queues" },
  { id: "why", label: "Why", hint: "Where the last number you clicked came from" },
  { id: "log", label: "Log", hint: "What the engine and this page have reported during this session" },
];

export function Inspector(): React.JSX.Element {
  const tab = useStudio((s) => s.inspectorTab);
  const setTab = useStudio((s) => s.setInspectorTab);
  const selectedNode = useStudio((s) => s.selectedNode);
  const hudDocked = useStudio((s) => s.hudDocked);
  const logs = useStudio((s) => s.logs.length);
  const label = selectedNode !== null ? engine.nodes.get(selectedNode)?.label || `node ${selectedNode}` : "Inspector";

  return (
    <>
      <div className="panel-head insp-head">
        <span className="insp-title" data-testid="inspector-title">
          {label}
        </span>
      </div>
      <div className="insp-tabs tabs" role="tablist" aria-label="Inspector">
        {TABS.map((t) => (
          <button
            key={t.id}
            type="button"
            role="tab"
            aria-selected={tab === t.id}
            className={tab === t.id ? "active" : ""}
            onClick={() => setTab(t.id)}
            data-testid={`tab-${t.id}`}
            title={t.hint}
          >
            {t.label}
            {t.id === "log" && logs > 0 ? <span className="count">{logs}</span> : null}
          </button>
        ))}
      </div>
      {tab === "state" ? <StateTab /> : null}
      {tab === "messages" ? <MessagePanel /> : null}
      {tab === "why" ? <WhyTab /> : null}
      {tab === "log" ? <LogTab /> : null}
      {hudDocked ? (
        <div className="panel-foot" style={{ display: "block", padding: 0 }}>
          <ObuHud docked />
        </div>
      ) : null}
    </>
  );
}
