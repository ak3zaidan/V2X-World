/**
 * The selected-vehicle HUD: the few things a person following a vehicle needs, and a door to the
 * rest.
 *
 * The owner's review (2026-10-06): "when a certain vehicle is selected it should only contain the
 * important information; drop any information that's not really relevant to the user". So the HUD
 * is five lines:
 *
 *  1. **Who** — what it is, and the pseudonym it is signing with now.
 *  2. **Moving** — speed, heading and position (the stream's own pose).
 *  3. **Radio** — messages sent and heard per second, how busy the channel is, and its neighbours.
 *  4. **Security** — how many valid certificates it holds, and whether it reaches its backend.
 *  5. **Health** — "healthy", or what is wrong, worst first (`lib/vehicleHealth.ts`, which cites the
 *     threshold behind each check).
 *
 * A value with nothing behind it, or a count at zero that says nothing, is not drawn. Everything
 * else — queues, CRL, verification, compute, GNSS, every message sent and received — is in the
 * vehicle overlay, which the Details button opens (`components/Inspector.tsx`).
 *
 * The HUD used to dock into the inspector column and every value opened a "why" tab; the column
 * became an overlay and the tab was removed, so both went.
 */

import { useMemo } from "react";

import { engine } from "../state/engine.js";
import { useStudio } from "../state/store.js";
import { int, shortDigest } from "../lib/format.js";
import { hasData, hudGroups, type HudField } from "../lib/telemetry.js";
import { bearingDeg } from "../lib/feed.js";
import { toGeodetic } from "../lib/geo.js";
import { linkText, pseudonymLine } from "../lib/security.js";
import { vehicleHealth } from "../lib/vehicleHealth.js";

/** One value with its label; nothing when the field has no data or reads zero. */
function Value({ field, label, keepZero = false }: { field: HudField | undefined; label: string; keepZero?: boolean }): React.JSX.Element | null {
  if (!hasData(field)) return null;
  if (!keepZero && field.raw === 0) return null;
  return (
    <span className="hud-field plain" data-testid={`hud-${field.key}`} title={field.help}>
      <span className="v">{field.value}</span>
      <span className="k">{label}</span>
    </span>
  );
}

function Plain({ k, v, testId, tone }: { k: string; v: string; testId: string; tone?: "warn" }): React.JSX.Element {
  return (
    <span className="hud-field plain" data-testid={testId}>
      <span className={tone === "warn" ? "v warn-text" : "v"}>{v}</span>
      <span className="k">{k}</span>
    </span>
  );
}

/** A labelled row, drawn only when something in it is. */
function Row({ title, testId, children }: { title: string; testId: string; children: (React.JSX.Element | null)[] }): React.JSX.Element | null {
  if (children.every((c) => c === null)) return null;
  return (
    <div className="hud-row" data-testid={testId}>
      <span className="hud-row-title">{title}</span>
      <span className="hud-row-body">{children}</span>
    </div>
  );
}

export function ObuHud(): React.JSX.Element | null {
  const telemetry = useStudio((s) => s.telemetry);
  const telemetryNode = useStudio((s) => s.telemetryNode);
  const pseudonym = useStudio((s) => s.pseudonym);
  const security = useStudio((s) => s.security);
  const inspect = useStudio((s) => s.inspect);
  const hello = useStudio((s) => s.hello);
  const pose = useStudio((s) => s.followedPose);
  const setOverlayOpen = useStudio((s) => s.setInspectorOpen);

  const byKey = useMemo(() => {
    const m = new Map<string, HudField>();
    if (telemetry) for (const g of hudGroups(telemetry)) for (const f of g.fields) m.set(f.key, f);
    return m;
  }, [telemetry]);
  const health = useMemo(() => vehicleHealth(telemetry, security), [telemetry, security]);

  const details = (
    <button
      type="button"
      className="small hud-details"
      onClick={() => setOverlayOpen(true)}
      data-testid="hud-details"
      title="Everything about this radio: every message it sent and received, its queues and its certificates (I)"
    >
      Details
    </button>
  );

  if (telemetryNode === null) {
    return (
      <div className="hud" data-testid="obu-hud">
        <div className="hud-head">
          <span className="id">No radio selected</span>
          <span className="dim">Select a vehicle or a roadside unit on the map.</span>
        </div>
      </div>
    );
  }

  const info = engine.nodes.get(telemetryNode);
  // A node that joined after the Hello is not in the Hello's node table (every vehicle that
  // spawns during a run), so its kind comes from the engine's own inspect.node answer.
  const inspected = inspect && Number(inspect.node) === Number(telemetryNode) ? inspect : null;
  const kind =
    info?.kind === 2 || inspected?.kind === "rsu"
      ? "Roadside unit"
      : info?.kind === 1 || inspected?.kind === "vru-device"
        ? "Pedestrian device"
        : "Vehicle";
  const name = info?.label || `node ${telemetryNode}`;
  const profile = info?.profileId || inspected?.profile_id || "";
  const line = pseudonymLine(security, pseudonym);

  const head = (
    <div className="hud-head">
      <span className="id" data-testid="hud-identity" title={profile ? `hardware profile ${profile}` : undefined}>
        {kind} {name}
      </span>
      {line ? (
        <span data-testid="hud-pseudonym" title={`pseudonym certificate ${line.digest}${line.tempId ? `, temporary id ${line.tempId}` : ""}`}>
          <span className="dim">pseudonym</span> <b>{shortDigest(line.digest)}</b>
          {line.indices ? <span className="dim"> ({line.indices})</span> : null}
        </span>
      ) : null}
      <span className="spacer grow" />
      {details}
    </div>
  );

  if (!telemetry) {
    return (
      <div className="hud" data-testid="obu-hud">
        {head}
        <div className="hud-body">
          {pose && hello ? <PoseRow pose={pose} origin={hello.origin} /> : null}
          <div className="hud-row dim">Waiting for its first radio report…</div>
        </div>
      </div>
    );
  }

  const val = (key: string, label: string, keepZero = false): React.JSX.Element | null => {
    const f = byKey.get(key);
    return hasData(f) && (keepZero || f.raw !== 0) ? <Value key={key} field={f} label={label} keepZero={keepZero} /> : null;
  };
  const pool = security && typeof security.pool_valid === "number" ? security.pool_valid : null;
  const link = security ? linkText(security) : null;

  return (
    <div className="hud" data-testid="obu-hud">
      {head}
      <div className="hud-body">
        {pose && hello ? <PoseRow pose={pose} origin={hello.origin} /> : null}
        <Row title="radio" testId="hud-row-radio">
          {[
            val("msgs_out_per_s", "sent", true),
            val("msgs_in_per_s", "heard"),
            val("cbr_pm", "channel busy"),
            val("nbr_total", "neighbours"),
          ]}
        </Row>
        <Row title="security" testId="hud-row-security">
          {[
            pool === null ? null : (
              <Plain key="pool" k={pool === 1 ? "valid certificate" : "valid certificates"} v={int(pool)} testId="hud-sec-pool" tone={pool === 0 ? "warn" : undefined} />
            ),
            link === null ? null : (
              <Plain key="link" k="backend" v={link} testId="hud-sec-link" tone={security?.link_up === false ? "warn" : undefined} />
            ),
          ]}
        </Row>
        <div className="hud-row" data-testid="hud-health">
          <span className="hud-row-title">health</span>
          <span className="hud-row-body">
            {health.length === 0 ? (
              <span className="ok-text" data-testid="hud-healthy" title="No dropped messages, the channel below the ETSI DCC busy threshold (CBR 0.30), valid certificates and the backend in reach.">
                healthy
              </span>
            ) : (
              health.map((h) => (
                <span key={h.key} className={h.level === "bad" ? "err-text" : "warn-text"} data-testid={`hud-issue-${h.key}`} title={h.why}>
                  {h.text}
                </span>
              ))
            )}
          </span>
        </div>
      </div>
    </div>
  );
}

/**
 * Where the followed vehicle is, as the stream draws it: speed, heading in the BSM's convention
 * (degrees clockwise from north) and latitude and longitude by the engine's own projection
 * (`lib/geo.ts`) — so it can be read against the position its broadcasts claim, which is its GNSS
 * belief and differs by the receiver's error.
 */
function PoseRow({
  pose,
  origin,
}: {
  pose: { x: number; y: number; speed: number; headingRad: number };
  origin: { lat: number; lon: number };
}): React.JSX.Element {
  const g = toGeodetic(origin, pose.x, pose.y);
  const heading = bearingDeg(pose.headingRad);
  const stopped = pose.speed < 0.1;
  return (
    <div className="hud-row" data-testid="hud-pose" title="the vehicle's true position in the stream, drawn at the body centre">
      <span className="hud-row-title">moving</span>
      <span className="hud-row-body">
        <span className="hud-field plain">
          <span className="v" data-testid="hud-pose-speed" data-value={pose.speed}>
            {stopped ? "stopped" : `${(pose.speed * 3.6).toFixed(0)} km/h`}
          </span>
        </span>
        <span className="hud-field plain">
          <span className="v" data-testid="hud-pose-heading" data-value={heading}>
            {heading.toFixed(0)}°
          </span>
          <span className="k">heading</span>
        </span>
        <span className="hud-field plain">
          <span className="v" data-testid="hud-pose-lat" data-value={g.lat}>
            {g.lat.toFixed(5)}
          </span>
          <span className="v" data-testid="hud-pose-lon" data-value={g.lon}>
            {g.lon.toFixed(5)}
          </span>
        </span>
      </span>
    </div>
  );
}
