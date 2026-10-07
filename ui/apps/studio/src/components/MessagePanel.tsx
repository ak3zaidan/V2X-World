/**
 * The followed radio's messages and queues, as the vehicle overlay shows them.
 *
 * The owner asked to see "the broadcast messages being sent out and their content, along with the
 * queue of that specific node" (2026-09-30), and then found the right-hand column too narrow to read
 * them in (2026-10-06). The column became an overlay over the viewport (`components/Inspector.tsx`),
 * and these tables were laid out for it: one column per thing a reader compares between rows.
 *
 *  * **Sent** — every frame the radio put on the air: time, type, the identity it carried (its
 *    temporary id and message count), what it said (speed, heading, position), its size, the power
 *    it went out at, and how long it took from generation to the air.
 *  * **Received** — every reception resolved at the radio: time, sender, type, its fate (delivered
 *    and verified, or lost and why), signal strength and SINR, the sender's distance, size, and the
 *    end-to-end delay.
 *  * **Queues** — the radio's five queues at this instant and the messages that passed through them.
 *
 * Clicking a row opens it in place, under the row: every field decoded from the frame's own octets,
 * the IEEE 1609.2 envelope, the delay stage by stage, and the SPDU in hex with its parts marked.
 * Opening a message pauses the feed while it is read, and closing it resumes (`lib/feed.ts`,
 * `readMessage`); the rows also hold still while the pointer is over them. A cell with nothing to
 * say is blank, not "n/a".
 */

import { Fragment, useEffect, useMemo } from "react";

import type { FeedAccess, FeedCrypto, FeedCryptoOp, FeedDecoded, FeedQueue, FeedReceived, FeedSent } from "@vwp/protocol";

import { useStudio } from "../state/store.js";
import { shortDigest, simClock } from "../lib/format.js";
import { fieldOf, fieldText, hexRows, keyOf, typesSeen, visibleRows, type FeedDir } from "../lib/feed.js";

/** Rows drawn per table; the ring keeps more, and the count says so. */
const DRAWN = 150;

/** A number with its unit, or blank when there is none: an empty cell, not "n/a". */
const fmt = (v: number | null | undefined, digits: number, unit = ""): string =>
  typeof v === "number" && Number.isFinite(v) ? `${v.toFixed(digits)}${unit}` : "";

/** A value or blank. */
const opt = (v: string | number | boolean | null | undefined): string => (v === null || v === undefined ? "" : String(v));

/** The stream clock without its hours, which a run of minutes never reaches. */
const clock = (tNs: number): string => simClock(tNs).slice(3);

/** The access layer's view of one transmission, on one line. */
function accessLine(a: FeedAccess): string {
  const parts = [`${a.rat} ${a.mcs}`];
  if (a.slot !== undefined) {
    parts.push(`slot ${a.slot}${a.subch !== undefined ? `, sub-channels ${a.subch}+${opt(a.subch_len)}` : ""}${a.subchannels !== undefined ? ` of ${a.subchannels}` : ""}`);
  }
  if (a.attempt !== undefined) parts.push(`transmission ${a.attempt}${a.attempts !== undefined ? ` of ${a.attempts}` : ""}`);
  if (a.cbr !== undefined) parts.push(`CBR ${a.cbr.toFixed(2)}`);
  if (a.cr !== undefined) parts.push(`CR ${a.cr.toFixed(3)}${a.cr_limit !== undefined ? ` (limit ${a.cr_limit.toFixed(3)})` : ""}`);
  return parts.join(" · ");
}

/** A decoded field for a table cell: blank when the message has no such field, "—" when its octets say "unavailable". */
function num(e: FeedSent | FeedReceived, k: string, digits: number, scale = 1): string {
  const f = fieldOf(e, k);
  if (!f) return "";
  if (f.na) return "—";
  return typeof f.v === "number" ? (f.v * scale).toFixed(digits) : opt(f.v);
}

/** Generation to the air, ms: the sum of the stages the engine timed, or the two instants' difference. */
function sendLatencyMs(e: FeedSent): number | null {
  const t = e.timing;
  if (typeof t.generated_ns === "number") return (t.on_air_ns - t.generated_ns) / 1e6;
  const parts = [t.sign_queue_ms, t.sign_ms, t.channel_access_ms].filter((x): x is number => typeof x === "number");
  return parts.length === 0 ? null : parts.reduce((a, b) => a + b, 0);
}

function fate(e: FeedReceived): string {
  if (e.outcome === "delivered") return e.verification ? `delivered, ${e.verification}` : "delivered";
  if (e.outcome === "in-flight") return "in flight";
  return `lost: ${(e.cause ?? "unknown").replace(/[_-]/g, " ")}`;
}

/** Row props shared by both tables: click or Enter/Space opens the message under the row. */
function rowProps(dir: FeedDir, key: string, openKey: string | null, read: (dir: FeedDir, key: string | null) => void): React.HTMLAttributes<HTMLTableRowElement> {
  const toggle = (): void => read(dir, openKey === key ? null : key);
  return {
    "aria-expanded": openKey === key,
    tabIndex: 0,
    onClick: toggle,
    onKeyDown: (ev) => {
      if (ev.key === "Enter" || ev.key === " ") {
        ev.preventDefault();
        toggle();
      }
    },
  };
}

const SENT_COLUMNS = 9;

function SentTable({ rows, openKey }: { rows: readonly FeedSent[]; openKey: string | null }): React.JSX.Element {
  const read = useStudio((s) => s.readFeedMessage);
  return (
    <table className="table feed-table" data-testid="feed-table-sent">
      <thead>
        <tr>
          <th>Time</th>
          <th>Type</th>
          <th title="The temporary id the message carried, and its message count">Identity</th>
          <th className="num" title="Speed the message reported">Speed</th>
          <th className="num" title="Heading the message reported, degrees from north">Heading</th>
          <th title="Position the message reported (the radio's GNSS belief)">Position</th>
          <th className="num" title="Octets on the air">Size</th>
          <th className="num" title="Transmit power">Power</th>
          <th className="num" title="From generation to the air: signing queue, signing and channel access">Latency</th>
        </tr>
      </thead>
      <tbody>
        {rows.slice(0, DRAWN).map((e, i) => {
          const key = keyOf("sent", e);
          const rotated = i + 1 < rows.length && rows[i + 1].pseudonym !== e.pseudonym;
          const lat = sendLatencyMs(e);
          return (
            <Fragment key={key}>
              <tr
                data-testid="feed-row-sent"
                data-msg={e.msg}
                className={`${openKey === key ? "open" : ""}${rotated ? " rotated" : ""}`}
                {...rowProps("sent", key, openKey, read)}
              >
                <td className="mono">{clock(e.t_ns)}</td>
                <td>
                  {e.type.toUpperCase()}
                  {rotated ? <span className="feed-badge" title="The pseudonym changed on this frame"> new identity</span> : null}
                </td>
                <td className="mono">{[opt(fieldOf(e, "temp_id")?.v), num(e, "msg_cnt", 0) === "" ? "" : `#${num(e, "msg_cnt", 0)}`].filter((x) => x !== "").join(" ")}</td>
                <td className="num">{fieldOf(e, "speed") ? `${num(e, "speed", 0, 3.6)} km/h` : ""}</td>
                <td className="num">{fieldOf(e, "heading") ? `${num(e, "heading", 0)}°` : ""}</td>
                <td className="mono">{fieldOf(e, "lat") ? `${num(e, "lat", 5)}, ${num(e, "lon", 5)}` : ""}</td>
                <td className="num">{e.bytes.on_wire} B</td>
                <td className="num">{fmt(e.radio.power_dbm, 0, " dBm")}</td>
                <td className="num">{fmt(lat, 2, " ms")}</td>
              </tr>
              {openKey === key ? (
                <tr className="feed-expand">
                  <td colSpan={SENT_COLUMNS}>
                    <Detail />
                  </td>
                </tr>
              ) : null}
            </Fragment>
          );
        })}
      </tbody>
    </table>
  );
}

const RECEIVED_COLUMNS = 9;

function ReceivedTable({ rows, openKey }: { rows: readonly FeedReceived[]; openKey: string | null }): React.JSX.Element {
  const read = useStudio((s) => s.readFeedMessage);
  return (
    <table className="table feed-table" data-testid="feed-table-received">
      <thead>
        <tr>
          <th>Time</th>
          <th title="The sending radio (ground truth), or the temporary id its message carried">From</th>
          <th>Type</th>
          <th>Outcome</th>
          <th className="num" title="Received signal strength">Signal</th>
          <th className="num" title="Signal to interference and noise ratio">SINR</th>
          <th className="num" title="Distance to the sender (ground truth)">Distance</th>
          <th className="num" title="Octets on the air">Size</th>
          <th className="num" title="End to end: generation at the sender to delivery here">Latency</th>
        </tr>
      </thead>
      <tbody>
        {rows.slice(0, DRAWN).map((e) => {
          const key = keyOf("received", e);
          const from = e.from !== undefined && e.from !== null ? `node ${e.from}` : opt(fieldOf(e, "temp_id")?.v);
          const outcome = e.outcome === "delivered" ? (e.verification === "unverified" ? "warn" : "ok") : e.outcome === "lost" ? "lost" : "";
          return (
            <Fragment key={key}>
              <tr
                data-testid="feed-row-received"
                data-outcome={e.outcome}
                className={`${openKey === key ? "open" : ""}${e.outcome === "delivered" ? "" : " lost"}`}
                {...rowProps("received", key, openKey, read)}
              >
                <td className="mono">{clock(e.t_ns)}</td>
                <td data-testid="feed-from">{from}</td>
                <td>{e.type.toUpperCase()}</td>
                <td>
                  <span className={`feed-outcome ${outcome}`}>{fate(e)}</span>
                </td>
                <td className="num">{fmt(e.rssi_dbm, 0, " dBm")}</td>
                <td className="num">{fmt(e.sinr_db, 1, " dB")}</td>
                <td className="num">{fmt(e.dist_m, 0, " m")}</td>
                <td className="num">{e.bytes_on_wire === null ? "" : `${e.bytes_on_wire} B`}</td>
                <td className="num">{fmt(e.e2e_ms, 2, " ms")}</td>
              </tr>
              {openKey === key ? (
                <tr className="feed-expand">
                  <td colSpan={RECEIVED_COLUMNS}>
                    <Detail />
                  </td>
                </tr>
              ) : null}
            </Fragment>
          );
        })}
      </tbody>
    </table>
  );
}

/** The SPDU in hex, sixteen octets a line, each octet tinted by the span it belongs to. */
function HexDump({ decoded }: { decoded: FeedDecoded }): React.JSX.Element | null {
  const rows = useMemo(() => hexRows(decoded.hex, decoded.spans, 16), [decoded.hex, decoded.spans]);
  if (rows.length === 0) return null;
  const spans = decoded.spans ?? [];
  const cls = (i: number): string => (i < 0 ? "" : `span-${spans[i].name.replace(/[^a-z0-9]+/gi, "-").toLowerCase()}`);
  return (
    <div className="feed-hex" data-testid="feed-hex">
      <h4>Bytes</h4>
      <div className="feed-hex-legend">
        {spans.map((s, i) => (
          <span key={s.name} className={`feed-hex-key ${cls(i)}`} data-testid={`feed-${cls(i)}`}>
            {s.name} <span className="faint">({s.end - s.start} B)</span>
          </span>
        ))}
      </div>
      <pre className="mono" aria-label={`the SPDU, ${decoded.spdu_bytes ?? rows.length * 16} octets in hex`}>
        {rows.map((r) => (
          <Fragment key={r.offset}>
            <span className="faint">{r.offset.toString(16).padStart(4, "0")} </span>
            {r.bytes.map((b) => (
              <span key={b.offset} className={cls(b.span)} title={b.span >= 0 ? spans[b.span].name : undefined}>
                {b.hex}{" "}
              </span>
            ))}
            {"\n"}
          </Fragment>
        ))}
      </pre>
    </div>
  );
}

function KV({ k, v, testid }: { k: string; v: string; testid?: string }): React.JSX.Element | null {
  if (v === "") return null;
  return (
    <>
      <dt>{k}</dt>
      <dd data-testid={testid}>{v}</dd>
    </>
  );
}

function Decoded({ d }: { d: FeedDecoded }): React.JSX.Element {
  const m = d.message;
  const sec = d.security;
  return (
    <>
      {d.note ? <p className="dim">{d.note}</p> : null}
      {d.error ? <p className="warn-text">{d.error}</p> : null}
      {m ? (
        <div className="feed-detail-col">
          <h4>Decoded fields · {m.format}</h4>
          {m.error ? <p className="warn-text">{m.error}</p> : null}
          {m.note ? <p className="dim">{m.note}</p> : null}
          <dl className="kv" data-testid="feed-fields">
            {m.fields.map((f) => (
              <Fragment key={f.k}>
                <dt>{f.label}</dt>
                <dd data-testid={`feed-field-${f.k}`} data-value={f.v === null ? "" : String(f.v)} className={f.na ? "hud-na" : undefined}>
                  {fieldText(f.v, f.unit, f.na)}
                  {typeof f.raw === "number" && f.unit ? <span className="faint"> ({f.raw})</span> : null}
                </dd>
              </Fragment>
            ))}
          </dl>
        </div>
      ) : null}
      {sec ? (
        <div className="feed-detail-col">
          <h4>Security envelope · {sec.standard}</h4>
          <dl className="kv" data-testid="feed-security">
            <KV k="PSID" v={`${sec.psid} (0x${sec.psid.toString(16)})`} />
            <KV k="generation time" v={sec.generation_time_us === null ? "" : `${sec.generation_time_us} µs since 2004-01-01`} />
            <KV k="signer" v={sec.signer.kind} testid="feed-signer-kind" />
            {sec.signer.hashed_id8 ? <KV k="HashedId8" v={sec.signer.hashed_id8} testid="feed-hashedid8" /> : null}
            {sec.signer.certificate ? (
              <>
                <KV k="certificate" v={`${sec.signer.certificate.type}, ${sec.signer.certificate.bytes ?? "?"} B, issuer ${sec.signer.certificate.issuer}`} />
                <KV k="validity" v={`from Time32 ${sec.signer.certificate.validity_start_time32} for ${sec.signer.certificate.validity_duration}`} />
                <KV k="permissions" v={`PSID ${sec.signer.certificate.app_permissions.join(", ")}`} />
                <KV k="CRL series" v={String(sec.signer.certificate.crl_series)} />
              </>
            ) : null}
            <KV k="signature" v={`${sec.signature.alg}, r ${shortDigest(sec.signature.r)}, s ${shortDigest(sec.signature.s)}`} />
          </dl>
        </div>
      ) : null}
    </>
  );
}

/** A crypto time with where it ran: "9.000 ms · signed in the secure element". */
function withPlacement(time: string, op: FeedCryptoOp | null | undefined): string {
  return op ? `${time} · ${op.label}` : time;
}

/**
 * Why sign and verify compare as they do on this node's hardware. On the reference OBU the
 * signature is made in a slow secure element and verification runs on a line-rate accelerator, so
 * verify reads faster than sign — the reverse of ECDSA on one processor, which the note says.
 */
function CryptoNote({ crypto }: { crypto: FeedCrypto | null }): React.JSX.Element | null {
  if (!crypto || (!crypto.sign && !crypto.verify)) return null;
  return (
    <p className="dim feed-crypto-note" data-testid="feed-crypto-note" title={`hardware profile ${crypto.profile}`}>
      {crypto.explanation}
    </p>
  );
}

/** The open message: how it travelled, what it said, and its bytes. */
function Detail(): React.JSX.Element | null {
  const open = useStudio((s) => s.feed.open);
  const read = useStudio((s) => s.readFeedMessage);
  const crypto = useStudio((s) => s.feed.crypto);
  if (!open) return null;
  const e = open.entry;
  const sent = open.dir === "sent" ? (e as FeedSent) : null;
  const rx = open.dir === "received" ? (e as FeedReceived) : null;
  return (
    <div className="feed-detail" data-testid="feed-detail" data-msg={String(e.msg)}>
      <div className="feed-detail-head">
        <b>
          {e.type.toUpperCase()}
          {e.msg !== undefined && e.msg !== null ? ` · message ${String(e.msg)}` : ""} · {open.dir === "sent" ? "sent" : "received"} at {simClock(e.t_ns)}
        </b>
        <button type="button" className="small" onClick={() => read(open.dir, null)} title="Close this message and resume the feed" data-testid="feed-detail-close">
          Close
        </button>
      </div>
      <div className="feed-detail-grid">
        <div className="feed-detail-col">
          <h4>{sent ? "Transmission" : "Reception"}</h4>
          {sent ? (
            <dl className="kv">
              <KV k="pseudonym" v={opt(sent.pseudonym)} />
              <KV k="signed with" v={opt(sent.signer)} />
              <KV
                k="size"
                v={[
                  sent.bytes.payload !== undefined && sent.bytes.payload !== null ? `payload ${sent.bytes.payload}` : "",
                  sent.bytes.envelope !== undefined && sent.bytes.envelope !== null
                    ? `security ${sent.bytes.envelope}${sent.bytes.certificate ? ` (certificate ${sent.bytes.certificate})` : ""}`
                    : "",
                  sent.bytes.network !== undefined && sent.bytes.network !== null ? `network ${sent.bytes.network}` : "",
                  sent.bytes.link !== undefined && sent.bytes.link !== null ? `link ${sent.bytes.link}` : "",
                ]
                  .filter((x) => x !== "")
                  .join(" + ")
                  .concat(` = ${sent.bytes.on_wire} B`)}
              />
              <KV
                k="radio"
                v={[
                  fmt(sent.radio.power_dbm, 1, " dBm"),
                  sent.radio.channel !== undefined && sent.radio.channel !== null ? `channel ${sent.radio.channel}` : "",
                  sent.radio.airtime_us !== undefined && sent.radio.airtime_us !== null ? `${sent.radio.airtime_us} µs on air` : "",
                ]
                  .filter((x) => x !== "")
                  .join(" · ")}
              />
              {sent.radio.access ? <KV k="access" v={accessLine(sent.radio.access)} /> : null}
              <KV k="signing queue" v={fmt(sent.timing.sign_queue_ms, 3, " ms")} />
              <KV k="signing" v={withPlacement(fmt(sent.timing.sign_ms, 3, " ms"), crypto?.sign)} testid="feed-sign-time" />
              {crypto?.sign ? <KV k="· figure" v={crypto.sign.basis_text} testid="feed-sign-basis" /> : null}
              <KV k="channel access" v={fmt(sent.timing.channel_access_ms, 3, " ms")} />
            </dl>
          ) : null}
          {rx ? (
            <dl className="kv">
              <KV k="from" v={rx.from !== undefined && rx.from !== null ? `node ${rx.from}` : "not shown on a node-profile connection"} />
              <KV k="outcome" v={fate(rx)} testid="feed-fate" />
              <KV
                k="signal"
                v={[fmt(rx.rssi_dbm, 1, " dBm"), rx.sinr_db !== undefined && rx.sinr_db !== null ? `SINR ${fmt(rx.sinr_db, 1, " dB")}` : "", fmt(rx.dist_m, 1, " m away")]
                  .filter((x) => x !== "")
                  .join(" · ")}
              />
              <KV k="end to end" v={fmt(rx.e2e_ms, 3, " ms")} />
              {Object.entries(rx.stages_ms).map(([k, v]) => (
                <KV
                  key={k}
                  k={`· ${k.replace(/_/g, " ")}`}
                  v={/^verif/.test(k) && !/wait|queue/.test(k) ? withPlacement(`${v.toFixed(3)} ms`, crypto?.verify) : `${v.toFixed(3)} ms`}
                />
              ))}
              {crypto?.verify ? <KV k="· verify figure" v={crypto.verify.basis_text} testid="feed-verify-basis" /> : null}
            </dl>
          ) : null}
          <CryptoNote crypto={crypto} />
        </div>
        {e.decoded ? (
          <Decoded d={e.decoded} />
        ) : (
          <p className="dim">This radio never had the octets: the frame was lost before it reached the application.</p>
        )}
      </div>
      {e.decoded ? <HexDump decoded={e.decoded} /> : null}
    </div>
  );
}

function QueueTable({ queues, stepMs }: { queues: readonly FeedQueue[]; stepMs: number }): React.JSX.Element {
  return (
    <>
      <table className="table feed-table" data-testid="feed-queues">
        <thead>
          <tr>
            <th>Queue</th>
            <th>What it holds</th>
            <th className="num" title={`Messages waiting at this instant / the most at once during the last ${stepMs} ms`}>Waiting now / peak</th>
            <th className="num" title="Being served now">In service</th>
            <th className="num" title="Wait, median over the last second">Wait p50</th>
            <th className="num" title="Wait, 95th percentile over the last second">Wait p95</th>
            <th title="Dropped over the last ten seconds, by cause">Dropped</th>
          </tr>
        </thead>
        <tbody>
          {queues.map((q) => (
            <tr key={q.id} data-testid={`queue-${q.id}`}>
              <td>{q.label}</td>
              <td className="dim">{q.what}</td>
              <td className="num" data-testid={`queue-${q.id}-depth`} data-depth={q.depth ?? ""} data-peak={q.peak ?? ""}>
                {q.depth === null ? "—" : `${q.depth} / ${q.peak ?? q.depth}`}
              </td>
              <td className="num">{q.in_service}</td>
              <td className="num">{fmt(q.wait_p50_ms, 2, " ms")}</td>
              <td className="num">{fmt(q.wait_p95_ms, 2, " ms")}</td>
              <td title={q.drops_note} className={Object.keys(q.drops).length > 0 ? "warn-text" : undefined} data-testid={`queue-${q.id}-drops`}>
                {Object.keys(q.drops).length === 0
                  ? q.drops_note
                    ? ""
                    : "0"
                  : Object.entries(q.drops)
                      .map(([k, v]) => `${k.replace(/_/g, " ")} ${v}`)
                      .join(", ")}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      {queues
        .filter((q) => q.waiting.length > 0)
        .map((q) => (
          <div className="section" key={q.id} data-testid={`queue-waiting-${q.id}`}>
            <h4>
              {q.label}: {q.depth ?? 0} waiting now, {q.waiting.length + q.waiting_omitted} passed through in the last {stepMs} ms
              {q.waiting_omitted > 0 ? ` (${q.waiting_omitted} not listed)` : ""}
            </h4>
            <table className="table feed-table">
              <thead>
                <tr>
                  <th>Message</th>
                  <th>Type</th>
                  <th>From</th>
                  <th>Queued at</th>
                  <th className="num">Waited</th>
                  <th>Now</th>
                </tr>
              </thead>
              <tbody>
                {q.waiting.map((w) => (
                  <tr
                    key={`${String(w.msg)}-${w.enqueued_ns}`}
                    data-testid="queue-entry"
                    data-waiting={w.left_ns === null ? "yes" : "no"}
                    className={w.left_ns === null ? undefined : "lost"}
                  >
                    <td className="mono">{opt(w.msg)}</td>
                    <td>{w.type.toUpperCase()}</td>
                    <td>{w.from === null ? "itself" : `node ${w.from}`}</td>
                    <td className="mono">{clock(w.enqueued_ns)}</td>
                    <td className="num">{w.waited_ms.toFixed(2)} ms</td>
                    <td>{w.left_ns === null ? w.stage : `left at ${simClock(w.left_ns).slice(6)}, ${w.stage}`}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ))}
    </>
  );
}

/**
 * The line over a table: whether the list is moving, the Pause button, and the filters. The
 * bookkeeping (rows the push limit left out, rows the ring let go, frames never detected) is in the
 * status line's tooltip.
 */
function FeedBar({ dir }: { dir: FeedDir | null }): React.JSX.Element {
  const feed = useStudio((s) => s.feed);
  const setPaused = useStudio((s) => s.setFeedPaused);
  const read = useStudio((s) => s.readFeedMessage);
  const setFilter = useStudio((s) => s.setFeedFilter);
  const types = typesSeen(feed);
  const latest = feed.sent[0];
  const toggleType = (t: string): void => {
    const on = new Set(feed.types);
    if (on.has(t)) on.delete(t);
    else on.add(t);
    setFilter({ types: [...on] });
  };
  const holding = feed.paused || feed.hovering;
  return (
    <div className="feed-bar">
      <button
        type="button"
        data-testid="feed-pause"
        aria-pressed={feed.paused}
        className={feed.paused ? "active" : undefined}
        onClick={() => {
          // Resuming a pause that opening a message made also closes the message.
          if (feed.paused && feed.pausedForReading && feed.open) read(feed.open.dir, null);
          else setPaused(!feed.paused);
        }}
        title={feed.paused ? "Show what arrived while paused and keep streaming" : "Freeze the tables; messages keep arriving and are shown on resume"}
      >
        {feed.paused ? `Resume${feed.held.length > 0 ? ` (${feed.held.length} waiting)` : ""}` : "Pause"}
      </button>
      <span
        className={`feed-status${holding ? " held" : ""}`}
        data-testid="feed-status"
        title={
          [
            feed.omitted.sent + feed.omitted.received > 0 ? `${feed.omitted.sent + feed.omitted.received} rows left out by the push limit` : "",
            feed.dropped.sent + feed.dropped.received > 0 ? `${feed.dropped.sent + feed.dropped.received} older rows dropped from the list` : "",
            feed.undetected > 0 ? `${feed.undetected} frames sent within earshot were never detected (out of range or below sensitivity)` : "",
          ]
            .filter((x) => x !== "")
            .join("\n") || undefined
        }
      >
        {feed.unavailable ? (
          <span className="warn-text">{feed.unavailable}</span>
        ) : feed.pushes === 0 ? (
          "waiting for the engine's first push…"
        ) : (
          <>
            {feed.paused ? (feed.pausedForReading ? "paused while you read a message" : "paused") : feed.hovering ? "held while the pointer is over the list" : "live"} at {simClock(feed.tNs)}
            {holding && feed.held.length > 0 ? ` · ${feed.held.length} update${feed.held.length === 1 ? "" : "s"} waiting` : ""}
            {latest?.pseudonym ? <> · pseudonym {shortDigest(latest.pseudonym)}</> : null}
          </>
        )}
      </span>
      <span className="grow" />
      {dir !== null && types.length > 1 ? (
        <span className="feed-filters" data-testid="feed-filters">
          {types.map((t) => (
            <button
              key={t}
              type="button"
              className={feed.types.length === 0 || feed.types.includes(t) ? "chip on" : "chip"}
              aria-pressed={feed.types.includes(t)}
              data-testid={`feed-type-${t}`}
              onClick={() => toggleType(t)}
            >
              {t.toUpperCase()}
            </button>
          ))}
        </span>
      ) : null}
      {dir === "received" ? (
        <label className="feed-filters">
          Show{" "}
          <select
            value={feed.outcome}
            data-testid="feed-outcome"
            onChange={(ev) => setFilter({ outcome: ev.target.value as "all" | "delivered" | "lost" })}
          >
            <option value="all">every outcome</option>
            <option value="delivered">delivered only</option>
            <option value="lost">lost only</option>
          </select>
        </label>
      ) : null}
    </div>
  );
}

/** The rows hold still while the pointer is over them; a list that unmounts under it never sees it leave. */
function useHoverHold(): { onPointerEnter: () => void; onPointerLeave: () => void } {
  const setHover = useStudio((s) => s.setFeedHover);
  useEffect(() => () => setHover(false), [setHover]);
  return { onPointerEnter: () => setHover(true), onPointerLeave: () => setHover(false) };
}

/** The open message when its row is not drawn (filtered out, or past the drawn rows): shown above the table. */
function OrphanDetail({ dir, drawnKeys }: { dir: FeedDir; drawnKeys: ReadonlySet<string> }): React.JSX.Element | null {
  const open = useStudio((s) => s.feed.open);
  if (!open || open.dir !== dir || drawnKeys.has(open.key)) return null;
  return <Detail />;
}

function NoRadio(): React.JSX.Element {
  return (
    <div className="panel-body" data-testid="message-panel">
      <p className="dim">Select a vehicle and every message it sends and receives appears here.</p>
    </div>
  );
}

/** One direction of the feed: the bar, the open message if its row is not drawn, and the table. */
export function MessageList({ dir }: { dir: FeedDir }): React.JSX.Element {
  const feed = useStudio((s) => s.feed);
  const selectedNode = useStudio((s) => s.selectedNode);
  const hold = useHoverHold();
  if (selectedNode === null) return <NoRadio />;
  const openKey = feed.open?.key ?? null;
  const rows: readonly (FeedSent | FeedReceived)[] = dir === "sent" ? visibleRows(feed, "sent") : visibleRows(feed, "received");
  const drawnKeys = new Set(rows.slice(0, DRAWN).map((e) => keyOf(dir, e)));
  return (
    <div className="panel-body feed" data-testid="message-panel">
      <FeedBar dir={dir} />
      <OrphanDetail dir={dir} drawnKeys={drawnKeys} />
      {rows.length === 0 ? (
        <p className="dim">{dir === "sent" ? "Nothing sent by this radio yet in the part of the run played so far." : "Nothing received that passes the filters."}</p>
      ) : (
        <div className="feed-scroll" data-testid={`feed-scroll-${dir}`} {...hold}>
          {dir === "sent" ? (
            <SentTable rows={rows as readonly FeedSent[]} openKey={openKey} />
          ) : (
            <ReceivedTable rows={rows as readonly FeedReceived[]} openKey={openKey} />
          )}
        </div>
      )}
      {rows.length > DRAWN ? <p className="faint">Showing the newest {DRAWN} of {rows.length}.</p> : null}
    </div>
  );
}

/** The radio's queues at this instant and what passed through them. */
export function QueueList(): React.JSX.Element {
  const feed = useStudio((s) => s.feed);
  const selectedNode = useStudio((s) => s.selectedNode);
  if (selectedNode === null) return <NoRadio />;
  return (
    <div className="panel-body feed" data-testid="message-panel">
      <FeedBar dir={null} />
      {feed.queues ? (
        <>
          <QueueTable queues={feed.queues.list} stepMs={feed.queues.step_ms} />
          <p className="help">
            {feed.queues.source}.
            {typeof feed.queues.kernel_lead_ms === "number" && feed.queues.kernel_lead_ms < 2 * feed.queues.step_ms
              ? ` The engine is only ${feed.queues.kernel_lead_ms.toFixed(0)} ms ahead of this instant, so a message still queued past that is not yet known and "now" may read low.`
              : ""}
          </p>
        </>
      ) : (
        <p className="dim">No queue reading yet.</p>
      )}
    </div>
  );
}
