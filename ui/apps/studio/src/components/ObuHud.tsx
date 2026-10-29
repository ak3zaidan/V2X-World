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

import { useEffect, useMemo, useState } from "react";
import { createPortal } from "react-dom";

import { engine } from "../state/engine.js";
import { useStudio } from "../state/store.js";
import { int, shortDigest } from "../lib/format.js";
import { hasData, hudGroups, type HudField } from "../lib/telemetry.js";
import { bearingDeg } from "../lib/feed.js";
import { toGeodetic } from "../lib/geo.js";
import { hasPkiPool, linkText, pseudonymLine, signingStatus } from "../lib/security.js";
import { CertBadge, StatusGlyph } from "./pki/CertStatus.js";
import { openVehicle, usePki } from "./pki-records/state.js";
import { VEHICLE_STATE } from "../lib/pkiCert.js";
import { CREDENTIAL_LABEL, GROUND_TRUTH, asVehicles, credentialState } from "../lib/pkiRecords.js";
import { openPanel } from "../shell/route.js";
import { vehicleHealth } from "../lib/vehicleHealth.js";
import { actorNoun } from "../lib/actorNoun.js";
import { deviceOfNode } from "../lib/pkiNames.js";
import { noteRadioSample } from "../lib/vehicleHistory.js";
import { VehicleHistory } from "./VehicleHistory.js";

const capitalise = (w: string): string => w.charAt(0).toUpperCase() + w.slice(1);

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
  const selectedActor = useStudio((s) => s.selectedActor);
  const selectedNode = useStudio((s) => s.selectedNode);
  const pseudonym = useStudio((s) => s.pseudonym);
  const security = useStudio((s) => s.security);
  const inspect = useStudio((s) => s.inspect);
  const hello = useStudio((s) => s.hello);
  const pose = useStudio((s) => s.followedPose);
  const feedNode = useStudio((s) => s.feed.node);
  const feedCrypto = useStudio((s) => s.feed.crypto);
  const setOverlayOpen = useStudio((s) => s.setInspectorOpen);

  const byKey = useMemo(() => {
    const m = new Map<string, HudField>();
    if (telemetry) for (const g of hudGroups(telemetry)) for (const f of g.fields) m.set(f.key, f);
    return m;
  }, [telemetry]);
  const health = useMemo(() => vehicleHealth(telemetry, security), [telemetry, security]);
  // Keep each once-a-second radio report for the vehicle's history (BSMs sent and heard).
  useEffect(() => {
    if (telemetryNode === null || !telemetry) return;
    const r = (k: string): number => {
      const f = byKey.get(k);
      return f && typeof f.raw === "number" ? f.raw : 0;
    };
    noteRadioSample(Number(telemetryNode), {
      t: useStudio.getState().simTimeNs,
      sent: r("msgs_out_per_s"),
      heard: r("msgs_in_per_s"),
      fullCert: r("full_cert_msgs"),
      digestMisses: r("digest_misses"),
      neighbours: r("nbr_total"),
      verified: r("nbr_verified"),
      revokedNeighbours: r("nbr_revoked"),
      cbr: r("cbr_pm"),
    });
  }, [telemetry, telemetryNode, byKey]);

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
          {selectedActor !== null ? (
            <>
              {/* The node's number when it has a radio: the number the camera chip and every certificate view use. */}
              <span className="id" data-testid="hud-identity-waiting">
                {capitalise(actorNoun(selectedActor))} {selectedNode ?? selectedActor}
              </span>
              <span className="dim">
                {selectedNode !== null ? "Waiting for its first radio report (once a second while the run plays)." : "Carries no radio, so there is no radio to report on."}
              </span>
            </>
          ) : (
            <>
              <span className="id">No radio selected</span>
              <span className="dim">Select a vehicle or a roadside unit on the map.</span>
            </>
          )}
        </div>
        {selectedNode !== null ? (
          <div className="hud-body">
            <HudCertificates node={selectedNode} />
          </div>
        ) : null}
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
  // A node with no label of its own is named by its number ("Vehicle 66"), as every certificate view names it.
  const name = info?.label && !/^node \d+$/.test(info.label) ? info.label : String(telemetryNode);
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
          <div className="hud-row dim">Waiting for its first radio report (once a second while the run plays).</div>
          {security ? null : <HudCertificates node={telemetryNode} />}
        </div>
      </div>
    );
  }

  const val = (key: string, label: string, keepZero = false): React.JSX.Element | null => {
    const f = byKey.get(key);
    return hasData(f) && (keepZero || f.raw !== 0) ? <Value key={key} field={f} label={label} keepZero={keepZero} /> : null;
  };
  const pool = security && typeof security.pool_valid === "number" ? security.pool_valid : null;
  // Read, not subscribed: the clock ticks every frame, and the row it is compared with arrives once a second.
  const signing = signingStatus(security, useStudio.getState().simTimeNs);
  const link = security ? linkText(security) : null;
  // Where this radio signs and verifies: on the reference OBU the two run on different
  // hardware, which is why verify reads faster than sign there.
  const crypto = feedCrypto && feedNode !== null && Number(feedNode) === Number(telemetryNode) ? feedCrypto : null;
  const cryptoText = crypto
    ? [crypto.sign ? `signs in ${crypto.sign.runs_on === "cpu" ? "software" : crypto.sign.runs_on === "hsm" ? "the secure element" : "the accelerator"}` : "", crypto.verify ? `verifies ${crypto.verify.runs_on === "cpu" ? "in software" : crypto.verify.runs_on === "hsm" ? "in the secure element" : "on the hardware accelerator"}` : ""]
        .filter((x) => x !== "")
        .join(", ")
    : null;

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
            signing === null ? null : (
              <span key="signing" className="hud-field plain" data-testid="hud-sec-status">
                <CertBadge status={signing} label={signing === "expired" && pool === 0 ? "no valid certificate" : undefined} />
              </span>
            ),
            pool === null ? null : (
              <Plain key="pool" k={pool === 1 ? "valid certificate" : "valid certificates"} v={int(pool)} testId="hud-sec-pool" tone={pool === 0 ? "warn" : undefined} />
            ),
            link === null ? null : (
              <Plain key="link" k="backend" v={link} testId="hud-sec-link" tone={security?.link_up === false ? "warn" : undefined} />
            ),
            hasPkiPool(security) && selectedNode !== null ? <HudActions key="actions" node={selectedNode} /> : null,
            cryptoText ? (
              <span key="crypto" className="hud-field plain" data-testid="hud-sec-crypto" title={crypto ? `${crypto.sign ? `sign: ${crypto.sign.basis_text}. ` : ""}${crypto.verify ? `verify: ${crypto.verify.basis_text}. ` : ""}${crypto.explanation}` : undefined}>
                <span className="v">{cryptoText}</span>
              </span>
            ) : null,
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
 * The security row from the vehicle's last certificate record, for the moments with no radio
 * report (a paused run, a vehicle just selected): its state in the certificates' shared language,
 * its valid pseudonym certificates, whether a revocation list names it, and the door to its pool.
 * Read from the PKI records (ground truth's vehicle list); nothing is drawn when the run has no
 * certificate system or ground truth is locked off.
 */
function HudCertificates({ node }: { readonly node: number }): React.JSX.Element | null {
  const locked = useStudio((s) => s.groundTruthLocked);
  const list = asVehicles(usePki(locked ? null : { topic: "vehicles", view: GROUND_TRUTH }, 3000).data);
  const row = list?.vehicles.find((v) => v.node === node) ?? null;
  if (!list || !row) return null;
  const st = credentialState(row, { nowNs: list.t_ns, offCtl: new Set(), crlOverdue: new Set(), groundTruth: true });
  const v = VEHICLE_STATE[st];
  const valid = typeof row.security?.pool_valid === "number" ? row.security.pool_valid : row.usable;
  const t = row.security?.t ?? row.t;
  return (
    <div className="hud-row" data-testid="hud-row-security" data-source="pki">
      <span className="hud-row-title">security</span>
      <span className="hud-row-body">
        <span className="hud-field plain" data-testid="hud-sec-state" data-state={st} title={v.meaning} style={{ color: v.color }}>
          <StatusGlyph shape={v.shape} /> <span className="v">{CREDENTIAL_LABEL[st]}</span>
        </span>
        <Plain k={valid === 1 ? "valid certificate" : "valid certificates"} v={int(valid)} testId="hud-sec-pool" tone={valid === 0 && st !== "revoked" ? "warn" : undefined} />
        <span className="hud-field plain">
          <span className="v">{st === "revoked" ? "on a revocation list" : "not revoked"}</span>
        </span>
        <span className="hud-asof" title="The vehicle publishes its certificate record when its certificates change">
          as of {(t / 1e9).toFixed(1)} s
        </span>
        <HudActions node={node} device={row.device} />
      </span>
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

/** The vehicle's two doors: its certificates in the certificate system view, and its full history. */
function HudActions({ node, device }: { readonly node: number; readonly device?: string }): React.JSX.Element {
  const [open, setOpen] = useState(false);
  const dev = device ?? deviceOfNode(node);
  return (
    <span className="hud-actions">
      <button
        type="button"
        className="hud-action"
        onClick={() => {
          openVehicle(node);
          openPanel("backend");
        }}
        data-testid="hud-sec-pki"
        title="Open its certificates in the certificate system view"
      >
        Certificates
      </button>
      <button type="button" className="hud-action" onClick={() => setOpen(true)} disabled={dev === null} data-testid="hud-history" title="Everything this vehicle did, step by step">
        History
      </button>
      {open && dev !== null ? createPortal(<VehicleHistory node={node} device={dev} onClose={() => setOpen(false)} />, document.body) : null}
    </span>
  );
}
