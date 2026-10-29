/**
 * The PDU decoder (00 §3.2 Flows tab): one SPDU, readable first — what kind of message it is in
 * plain words, who sent it to whom, its size, who signed it and who can decrypt it — then its
 * ASN.1 value tree with a short gloss beside each field the decoder knows, and its raw bytes
 * behind a toggle, as `inspect.pdu` answers it for the chosen view.
 *
 * The tree is the engine's: every layer the real decoder reads (`v2xw_proto::scms::pki_view::
 * decode_tree`), each in JER (ITU-T X.697), which `tools/pki_view/check_jer.py` checks against an
 * independent decoder (asn1tools over the unchanged 1609.2 modules). The decoder draws every leaf
 * of that JER once, in order, and nothing else (`lib/pki.ts` `treeLines`). An encrypted layer is
 * drawn as its ciphertext; the plaintext its recipient opened is drawn only in ground truth and as
 * that recipient — the engine withholds it from every other view, and the decoder says who could
 * read it.
 */

import { useEffect, useState } from "react";

import { GROUND_TRUTH, formatBytes, formatSimT, hexRows, pduOf, treeLines, type PduAnswer, type PduLayer } from "../../lib/pki.js";
import { fieldGloss, pduText } from "../../lib/pkiPdu.js";
import { grouped } from "../../lib/pkiCert.js";
import { entityLabel } from "../../lib/pkiRecords.js";
import { Term } from "./Term.js";
import { engine } from "../../state/engine.js";
import { CloseIcon } from "../../shell/Icons.js";

/** A JER value as an indented tree, every leaf once; with `explain`, a gloss beside known fields. */
export function JerTree({ jer, testId, explain = false }: { readonly jer: unknown; readonly testId?: string; readonly explain?: boolean }): React.JSX.Element {
  const lines = treeLines(jer);
  return (
    <div className="pki-tree mono" data-testid={testId}>
      {lines.slice(1).map((l) => {
        const gloss = explain ? fieldGloss(l.key) : null;
        return (
          <div key={l.path} className="pki-tree-line" style={{ paddingLeft: `${(l.depth - 1) * 12}px` }} data-path={l.path} data-leaf={l.value !== null ? "1" : undefined} title={gloss ?? undefined}>
            <span className="k">{l.key}</span>
            {l.value !== null ? <LeafValue k={l.key} value={l.value} /> : <span className="faint">{l.children === 0 ? "{}" : ""}</span>}
            {gloss ? <span className="g">{gloss}</span> : null}
          </div>
        );
      })}
    </div>
  );
}

/** Hex longer than this many bytes is folded to its size until asked for. */
const FOLD_BYTES = 48;

/**
 * One leaf's value. A long run of bytes (a ciphertext, a signature blob) reads as its size and
 * what it is, "694 B encrypted to its recipient", with the bytes one click away, never as a wall.
 */
function LeafValue({ k, value }: { readonly k: string; readonly value: string }): React.JSX.Element {
  const [open, setOpen] = useState(false);
  const m = /^"([0-9a-fA-F]+)"$/.exec(value);
  const bytes = m ? m[1].length / 2 : 0;
  if (!m || bytes <= FOLD_BYTES) return <span className="v">{value}</span>;
  const what = /cipher/i.test(k) ? "encrypted: only its recipient can read it" : "of data";
  return (
    <span className="v" data-folded={open ? "0" : "1"}>
      <button type="button" className="linklike pki-leaf-fold" onClick={() => setOpen((o) => !o)} aria-expanded={open} data-testid="pdu-leaf-fold">
        {bytes.toLocaleString()} B {what} ({open ? "hide bytes" : "show bytes"})
      </button>
      {open ? <span className="pki-leaf-bytes">{value}</span> : null}
    </span>
  );
}

/** Bytes as an offset-hex dump. */
export function HexDump({ hex, testId }: { readonly hex: string; readonly testId?: string }): React.JSX.Element {
  return (
    <pre className="pki-hex mono" data-testid={testId}>
      {hexRows(hex)
        .map((r) => `${r.offset}  ${r.bytes}`)
        .join("\n")}
    </pre>
  );
}

function Layer({ layer, index, raw }: { readonly layer: PduLayer; readonly index: number; readonly raw: boolean }): React.JSX.Element {
  const t = pduText(layer.type);
  return (
    <section className="pki-layer" data-testid={`pdu-layer-${index}`} data-type={layer.type}>
      <h4>
        <span>{t.what.replace(/^./, (c) => c.toUpperCase())}</span>
        <span className="dim mono">{layer.type}</span>
        {layer.content ? (
          <span className="pill" title="Which kind of content this envelope carries">
            {layer.content}
          </span>
        ) : null}
        <span className="faint mono" title={`Where this layer sits in the message: ${layer.path}`}>
          {layer.len} B
        </span>
      </h4>
      <div className="pki-layer-body">
        {raw ? <HexDump hex={layer.hex} testId={`pdu-hex-${index}`} /> : null}
        {layer.jer !== undefined ? (
          <JerTree jer={layer.jer} testId={`pdu-tree-${index}`} explain />
        ) : (
          <p className="faint">Not an ASN.1 type the decoder knows: {raw ? "shown as bytes above" : "turn on “raw bytes” to see them"}.</p>
        )}
      </div>
    </section>
  );
}

/** "the device's butterfly key", "provider-a/aca/0" → "ACA (provider-a)". */
function sealedText(to: string): string {
  return to === "ee" ? "the vehicle alone (its butterfly key: no backend key opens it)" : entityLabel(to);
}

/** The decoder drawer for one PDU id. */
export function PduDecoder({ id, view, onClose }: { readonly id: string; readonly view: string; readonly onClose: () => void }): React.JSX.Element {
  const [pdu, setPdu] = useState<PduAnswer | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [raw, setRaw] = useState(false);
  useEffect(() => {
    let live = true;
    setPdu(null);
    setError(null);
    engine
      .request("inspect.pdu", { id, view }, { quiet: true, timeoutMs: 8000 })
      .then((a) => {
        if (!live) return;
        const p = pduOf(a);
        setPdu(p);
        if (!p) setError("The engine answered, but not with a PDU.");
      })
      .catch((err: unknown) => live && setError(err instanceof Error ? err.message : String(err)));
    return () => {
      live = false;
    };
  }, [id, view]);

  const what = pdu ? pduText(pdu.type) : null;
  return (
    <aside className="pki-drawer pki-decoder" data-testid="pdu-decoder">
      <div className="pki-drawer-head">
        <h3>
          Message decoder
          {pdu ? <span className="dim mono pki-pdu-type"> {pdu.type}</span> : null}
        </h3>
        <span className="grow" />
        <button type="button" className="icon-button" onClick={onClose} aria-label="Close the decoder" data-testid="pdu-close">
          <CloseIcon />
        </button>
      </div>
      {error ? <p className="faint pki-pad">The engine could not decode this message: {error}</p> : null}
      {!pdu && !error ? <p className="faint pki-pad">Decoding…</p> : null}
      {pdu && what ? (
        <div className="pki-drawer-body">
          <p className="pki-pdu-what" data-testid="pdu-what">
            {what.what}
            {what.flow ? <span className="dim"> ({what.flow})</span> : null}.
          </p>
          <dl className="pki-pairs">
            <dt>From → to</dt>
            <dd title={`${pdu.from} → ${pdu.to}`}>
              {entityLabel(pdu.from)} → {entityLabel(pdu.to)}
            </dd>
            <dt>Size</dt>
            <dd className="mono">
              {pdu.len} B{pdu.len >= 1000 ? ` (${formatBytes(pdu.len)})` : ""}
            </dd>
            {pdu.id8 ? (
              <>
                <dt title="HashedId8: the low 8 bytes of its SHA-256, how lists and messages refer to it">
                  <Term k="hashedid8">Its id (HashedId8)</Term>
                </dt>
                <dd className="mono">{grouped(pdu.id8)}</dd>
              </>
            ) : null}
            <dt>Built at</dt>
            <dd className="mono" title={`${(pdu.t / 1e9).toFixed(6)} s`}>
              {formatSimT(pdu.t)}
            </dd>
            {pdu.sealed_to ? (
              <>
                <dt>Readable by</dt>
                <dd>{sealedText(pdu.sealed_to)}</dd>
              </>
            ) : null}
            <dt title="Ground truth sees every byte; an entity sees only what it sent or received, and only the plaintext it opened">Seen as</dt>
            <dd className="mono" data-testid="pdu-view" title={pdu.view === GROUND_TRUTH ? "everything (ground truth)" : entityLabel(pdu.view)}>
              {pdu.view === GROUND_TRUTH ? "ground truth" : pdu.view}
            </dd>
            <dt>Message id</dt>
            <dd className="mono faint">{pdu.id}</dd>
          </dl>
          <p className="faint pki-pdu-how">
            Below, each layer of the message from the outside in: every field with what it means beside it (hover a field for more). Encodings are{" "}
            <Term k="jer">JER</Term>; the raw bytes are one switch away.
          </p>
          <label className="pki-raw-toggle">
            <input type="checkbox" checked={raw} disabled={pdu.hex === null} onChange={(e) => setRaw(e.target.checked)} data-testid="pdu-raw-toggle" /> show the raw bytes (hex)
            above each layer's fields
          </label>
          {pdu.hex === null ? <p className="faint">The run kept this message's length only, not its bytes, so there is no hex to show.</p> : null}
          {pdu.layers.map((l, i) => (
            <Layer key={l.path} layer={l} index={i} raw={raw} />
          ))}
          {pdu.opened ? (
            "hidden" in pdu.opened ? (
              <section className="pki-plaintext hidden" data-testid="pdu-plaintext-hidden">
                <h4>Inside (encrypted)</h4>
                <p className="dim">{pdu.opened.hidden}.</p>
              </section>
            ) : (
              <section className="pki-plaintext" data-testid="pdu-plaintext">
                <h4>
                  Inside <span className="dim">as {pdu.opened.by} decrypted it with its own key</span>
                </h4>
                {pdu.opened.error ? <p className="warn">It did not open: {pdu.opened.error}</p> : null}
                {pdu.opened.layers.map((l, i) => (
                  <Layer key={`p${l.path}`} layer={l} index={100 + i} raw={raw} />
                ))}
              </section>
            )
          ) : null}
        </div>
      ) : null}
    </aside>
  );
}
