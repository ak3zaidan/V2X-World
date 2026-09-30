/**
 * The chase HUD: what the followed radio is doing, in the four things a researcher reads while
 * following a vehicle — who it is (its pseudonym), its radio (what it sends and hears, the channel
 * and its neighbours), its credentials (the certificate pool, the next top-up, the CRL) and its
 * queues — over five sparklines.
 *
 * It used to be the 09-ui §5 sketch line for line: six rows and 45 values, of which a third read
 * "n/a" on most runs (the evidence buffer, the HSM, the clock offset…), and a pseudonym line that
 * said "(indices pending — node.tx)" for a whole SCMS run. Now a value with nothing behind it is not
 * drawn, a row with nothing in it is not drawn, and the pseudonym's indices come from the node's own
 * `node.security` row (`lib/security.ts`). Everything the old HUD showed is still in the inspector's
 * overview, grouped the same way.
 *
 * Every telemetry value is a button: clicking it — or tabbing to it and pressing Enter or Space —
 * opens the inspector's "why" tab for that field (09-ui §10, keyboard control).
 */

import { useMemo } from "react";

import { Sparkline } from "./Sparkline.js";
import { engine } from "../state/engine.js";
import { useStudio } from "../state/store.js";
import { NA, int, shortDigest, simClock } from "../lib/format.js";
import { SPARKLINE_SERIES, hudGroups, totalDrops, type HudField } from "../lib/telemetry.js";
import { bearingDeg } from "../lib/feed.js";
import { toGeodetic } from "../lib/geo.js";
import { linkText, pseudonymLine, untilText, type NodeSecurityRow } from "../lib/security.js";

/** Whether a field has something to show: not at its "not modelled" sentinel. */
export function hasData(f: HudField | undefined): f is HudField {
  return f !== undefined && f.raw !== null && !f.value.includes(NA);
}

function fieldIndex(fields: HudField[]): Map<string, HudField> {
  const m = new Map<string, HudField>();
  for (const f of fields) m.set(f.key, f);
  return m;
}

/**
 * One clickable value, or nothing when the field has no data. `label` overrides the field's own.
 *
 * A real `<button>`, not a `<span onClick>`: each is the entry point to the inspector's "why" tab,
 * so it must be reachable by keyboard (WCAG 2.1 SC 2.1.1) and announced as a control (SC 4.1.2).
 * The help sits in a visually hidden `aria-describedby` target rather than a `title`.
 */
function Value({ field, label, node }: { field: HudField | undefined; label?: string; node: number | null }): React.JSX.Element | null {
  const setWhy = useStudio((s) => s.setWhy);
  if (!hasData(field)) return null;
  const name = label ?? field.label;
  const helpId = `hud-help-${field.key}`;
  return (
    <button
      type="button"
      className="hud-field"
      data-testid={`hud-${field.key}`}
      aria-label={`${name}: ${field.value}${field.visibility === "GT" ? " (ground truth)" : ""}`}
      aria-describedby={helpId}
      onClick={() =>
        setWhy({
          kind: "node_field",
          id: field.key,
          label: name,
          ...(node !== null ? { node } : {}),
          value: field.value,
          unit: field.unit,
        })
      }
    >
      <span className="k">{name}</span>
      <span className="v">{field.value}</span>
      {field.visibility === "GT" ? <span className="gt-tag">GT</span> : null}
      <span className="sr-only" id={helpId}>
        {field.key} — {field.unit}
        {field.help ? ` · ${field.help}` : ""} · activate to open the provenance tab
      </span>
    </button>
  );
}

/** A value that is not a telemetry field (the security row's): shown, not explained. */
function Plain({ k, v, testId, tone }: { k: string; v: string; testId: string; tone?: "warn" }): React.JSX.Element {
  return (
    <span className="hud-field plain" data-testid={testId}>
      <span className="k">{k}</span>
      <span className={tone === "warn" ? "v warn-text" : "v"}>{v}</span>
    </span>
  );
}

/** A labelled row, drawn only when something in it is. */
function Row({ title, testId, children }: { title: string; testId: string; children: (React.JSX.Element | null)[] }): React.JSX.Element | null {
  if (children.every((c) => c === null)) return null;
  return (
    <div className="hud-row" data-testid={testId}>
      <span className="hud-row-title">{title}</span>
      {children}
    </div>
  );
}

/** The security row's facts, in words; `null` members are not drawn. */
function securityFacts(row: NodeSecurityRow | null, nowNs: number): {
  pool: string | null;
  validUntil: string | null;
  topup: string | null;
  link: string | null;
  crl: string | null;
  revoked: boolean;
} {
  if (row === null) return { pool: null, validUntil: null, topup: null, link: null, crl: null, revoked: false };
  const pool =
    typeof row.pool_valid === "number"
      ? `${int(row.pool_valid)} valid${typeof row.pool_preloaded === "number" && row.pool_preloaded > 0 ? ` + ${int(row.pool_preloaded)} ahead` : ""}`
      : null;
  const topup = row.topup_in_flight === true ? "in flight" : untilText(row.next_topup ?? null, nowNs);
  const crl =
    typeof row.crl_entries === "number"
      ? `${int(row.crl_entries)} entr${row.crl_entries === 1 ? "y" : "ies"}${typeof row.crl_version === "number" && row.crl_version > 0 ? ` · v${row.crl_version}` : ""}`
      : null;
  return {
    pool,
    validUntil: untilText(row.cert_valid_until ?? null, nowNs),
    topup,
    link: linkText(row),
    crl,
    revoked: row.self_revoked === true,
  };
}

export function ObuHud({ docked = false }: { docked?: boolean }): React.JSX.Element | null {
  const telemetry = useStudio((s) => s.telemetry);
  const telemetryNode = useStudio((s) => s.telemetryNode);
  const pseudonym = useStudio((s) => s.pseudonym);
  const security = useStudio((s) => s.security);
  const inspect = useStudio((s) => s.inspect);
  const simTimeNs = useStudio((s) => s.simTimeNs);
  const seriesTick = useStudio((s) => s.seriesTick);
  const hello = useStudio((s) => s.hello);
  const pose = useStudio((s) => s.followedPose);
  const hudDocked = useStudio((s) => s.hudDocked);
  const setHudDocked = useStudio((s) => s.setHudDocked);

  const groups = useMemo(() => (telemetry ? hudGroups(telemetry) : []), [telemetry]);
  const byKey = useMemo(() => fieldIndex(groups.flatMap((g) => [...g.fields])), [groups]);

  const dock = (
    <button
      type="button"
      className="icon-button hud-dock"
      onClick={() => setHudDocked(!hudDocked)}
      data-testid="hud-dock"
      aria-label={hudDocked ? "Float the HUD over the viewport" : "Dock the HUD into the inspector"}
      title={hudDocked ? "Float over the viewport" : "Dock into the inspector"}
    >
      {hudDocked ? "float" : "dock"}
    </button>
  );

  if (telemetryNode === null) {
    return (
      <div className={`hud${docked ? " docked" : ""}`} data-testid="obu-hud">
        <div className="hud-head">
          <span className="id">No radio selected</span>
          <span className="dim">Select a vehicle or a roadside unit on the map.</span>
          <span className="spacer grow" />
          {dock}
        </div>
      </div>
    );
  }

  const info = engine.nodes.get(telemetryNode);
  // A node that joined after the Hello is not in the Hello's node table (every vehicle that
  // spawns during a run), so its kind and profile come from the engine's own inspect.node answer.
  const inspected = inspect && Number(inspect.node) === Number(telemetryNode) ? inspect : null;
  const hudKind =
    info?.kind === 2 || inspected?.kind === "rsu"
      ? "RSU"
      : info?.kind === 1 || inspected?.kind === "vru-device"
        ? "VRU"
        : "OBU";
  const name = info?.label || `node ${telemetryNode}`;
  const profile = info?.profileId || inspected?.profile_id || "";
  const line = pseudonymLine(security, pseudonym);

  if (!telemetry) {
    return (
      <div className={`hud${docked ? " docked" : ""}`} data-testid="obu-hud">
        <div className="hud-head">
          <span className="id" data-testid="hud-identity">
            {hudKind} {name}
          </span>
          <span className="dim">Waiting for its first report from the engine…</span>
          <span className="spacer grow" />
          {dock}
        </div>
      </div>
    );
  }

  const drops = totalDrops(telemetry);
  const sec = securityFacts(security, simTimeNs);
  const node = telemetryNode;
  // Built here, and `null` when empty, so a row can tell that it has nothing to draw.
  const val = (key: string, label: string): React.JSX.Element | null => {
    const f = byKey.get(key);
    return hasData(f) ? <Value key={key} field={f} label={label} node={node} /> : null;
  };
  const plain = (k: string, v: string | null, testId: string): React.JSX.Element | null =>
    v === null ? null : <Plain key={testId} k={k} v={v} testId={testId} />;

  return (
    <div className={`hud${docked ? " docked" : ""}`} data-testid="obu-hud">
      <div className="hud-head">
        <span className="id" data-testid="hud-identity" title={profile ? `profile ${profile}` : undefined}>
          {hudKind} {name}
        </span>
        {line ? (
          <span data-testid="hud-pseudonym" title={`pseudonym certificate ${line.digest}${line.tempId ? `, temporary id ${line.tempId}` : ""}`}>
            <span className="dim">pseudonym</span> <b>{shortDigest(line.digest)}</b>
            {line.indices ? <span className="dim"> ({line.indices})</span> : null}
          </span>
        ) : null}
        {sec.revoked ? (
          <span className="warn-text" data-testid="hud-self-revoked">
            on the CRL — stopped sending
          </span>
        ) : null}
        <span className="spacer grow" />
        <span className="dim" data-testid="hud-simtime">
          {simClock(simTimeNs)}
        </span>
        {dock}
      </div>

      <div className="hud-body">
        {pose && hello ? <PoseRow pose={pose} origin={hello.origin} /> : null}
        <Row title="radio" testId="hud-row-radio">
          {[
            val("msgs_out_per_s", "tx"),
            val("msgs_in_per_s", "rx"),
            val("cbr_pm", "CBR"),
            val("nbr_total", "neighbours"),
            val("dcc_state", "DCC"),
            val("tx_power_cdbm", "power"),
          ]}
        </Row>
        <Row title="security" testId="hud-row-security">
          {[
            plain("certs", sec.pool, "hud-sec-pool"),
            plain("next top-up", sec.topup, "hud-sec-topup"),
            plain("CRL", sec.crl, "hud-sec-crl"),
            plain("backend", sec.link, "hud-sec-link"),
            val("verifications_per_s", "verify"),
            val("unverified_ratio_pm", "unverified"),
          ]}
        </Row>
        <Row title="queues" testId="hud-row-queues">
          {[
            val("q_verify_p95", "verify p95"),
            val("verify_wait_p95_ms", "wait p95"),
            val("cpu_util_pm", "CPU"),
            drops > 0 ? (
              <span key="drops" className="hud-field plain" title="Messages dropped this window, all causes; the inspector breaks them down">
                <span className="k">dropped</span>
                <span className="v warn-text" data-testid="hud-drops">
                  {int(drops)}
                </span>
              </span>
            ) : null,
          ]}
        </Row>
      </div>

      <div className="sparkrow" data-testid="hud-sparklines">
        {SPARKLINE_SERIES.map((s, i) => (
          <Sparkline key={s.key} seriesIndex={i} label={s.label} unit={s.unit} tick={seriesTick} fieldKey={s.key} node={telemetryNode} />
        ))}
      </div>
    </div>
  );
}

/**
 * Where the followed vehicle is, as the stream draws it: latitude and longitude by the engine's own
 * projection (`lib/geo.ts`), speed, and heading in the BSM's convention (degrees clockwise from north)
 * — so it can be read against the position its broadcasts claim in the message panel, which is its
 * GNSS belief and differs by the receiver's error.
 */
function PoseRow({
  pose,
  origin,
}: {
  pose: { x: number; y: number; speed: number; headingRad: number };
  origin: { lat: number; lon: number };
}): React.JSX.Element {
  const g = toGeodetic(origin, pose.x, pose.y);
  return (
    <div className="hud-row" data-testid="hud-pose" title="the vehicle's pose in the stream (its true position, drawn at the body centre)">
      <span className="hud-row-title">where</span>
      <span className="hud-pose-field">
        <span className="v" data-testid="hud-pose-lat" data-value={g.lat}>
          {g.lat.toFixed(6)}°
        </span>
        <span className="v" data-testid="hud-pose-lon" data-value={g.lon}>
          {g.lon.toFixed(6)}°
        </span>
      </span>
      <span className="hud-pose-field">
        <span className="v" data-testid="hud-pose-speed" data-value={pose.speed}>
          {pose.speed.toFixed(1)} m/s
        </span>
      </span>
      <span className="hud-pose-field">
        <span className="v" data-testid="hud-pose-heading" data-value={bearingDeg(pose.headingRad)}>
          {bearingDeg(pose.headingRad).toFixed(0)}°
        </span>
      </span>
    </div>
  );
}
