/**
 * The PDU decoder (00 §3.2 Flows tab): one SPDU's bytes beside its ASN.1 value tree, as
 * `inspect.pdu` answers it for the chosen view.
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

import { GROUND_TRUTH, hexRows, pduOf, treeLines, type PduAnswer, type PduLayer } from "../../lib/pki.js";
import { engine } from "../../state/engine.js";
import { CloseIcon } from "../../shell/Icons.js";

/** A JER value as an indented tree, every leaf once. */
export function JerTree({ jer, testId }: { readonly jer: unknown; readonly testId?: string }): React.JSX.Element {
  const lines = treeLines(jer);
  return (
    <div className="pki-tree mono" data-testid={testId}>
      {lines.slice(1).map((l) => (
        <div key={l.path} className="pki-tree-line" style={{ paddingLeft: `${(l.depth - 1) * 12}px` }} data-path={l.path} data-leaf={l.value !== null ? "1" : undefined}>
          <span className="k">{l.key}</span>
          {l.value !== null ? <span className="v">{l.value}</span> : <span className="faint">{l.children === 0 ? "{}" : ""}</span>}
        </div>
      ))}
    </div>
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

function Layer({ layer, index }: { readonly layer: PduLayer; readonly index: number }): React.JSX.Element {
  return (
    <section className="pki-layer" data-testid={`pdu-layer-${index}`} data-type={layer.type}>
      <h4>
        <span className="mono">{layer.type}</span>
        {layer.content ? <span className="pill">{layer.content}</span> : null}
        <span className="dim mono">
          {layer.path} · {layer.len} B
        </span>
      </h4>
      <div className="pki-layer-body">
        <HexDump hex={layer.hex} />
        {layer.jer !== undefined ? <JerTree jer={layer.jer} testId={`pdu-tree-${index}`} /> : <p className="faint">Not an ASN.1 type the decoder knows: shown as bytes.</p>}
      </div>
    </section>
  );
}

/** The decoder drawer for one PDU id. */
export function PduDecoder({ id, view, onClose }: { readonly id: string; readonly view: string; readonly onClose: () => void }): React.JSX.Element {
  const [pdu, setPdu] = useState<PduAnswer | null>(null);
  const [error, setError] = useState<string | null>(null);
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

  return (
    <aside className="pki-drawer pki-decoder" data-testid="pdu-decoder">
      <div className="pki-drawer-head">
        <h3 className="mono">{pdu?.type ?? id}</h3>
        <span className="grow" />
        <button type="button" className="icon-button" onClick={onClose} aria-label="Close the decoder" data-testid="pdu-close">
          <CloseIcon />
        </button>
      </div>
      {error ? <p className="faint">The engine said: {error}</p> : null}
      {pdu ? (
        <div className="pki-drawer-body">
          <dl className="pki-pairs">
            <dt>id</dt>
            <dd className="mono">{pdu.id}</dd>
            <dt>from → to</dt>
            <dd className="mono">
              {pdu.from} → {pdu.to}
            </dd>
            <dt>length</dt>
            <dd className="mono">{pdu.len} B</dd>
            {pdu.id8 ? (
              <>
                <dt>HashedId8</dt>
                <dd className="mono">{pdu.id8}</dd>
              </>
            ) : null}
            <dt>built at</dt>
            <dd className="mono">{(pdu.t / 1e9).toFixed(6)} s</dd>
            {pdu.sealed_to ? (
              <>
                <dt>encrypted to</dt>
                <dd className="mono">{pdu.sealed_to === "ee" ? "the device's butterfly key (no backend key opens it)" : pdu.sealed_to}</dd>
              </>
            ) : null}
            <dt>view</dt>
            <dd className="mono" data-testid="pdu-view">
              {pdu.view === GROUND_TRUTH ? "ground truth" : pdu.view}
            </dd>
          </dl>
          {pdu.hex === null ? <p className="faint">The run kept this PDU's length only, not its bytes.</p> : null}
          {pdu.layers.map((l, i) => (
            <Layer key={l.path} layer={l} index={i} />
          ))}
          {pdu.opened ? (
            "hidden" in pdu.opened ? (
              <section className="pki-plaintext hidden" data-testid="pdu-plaintext-hidden">
                <h4>Plaintext</h4>
                <p className="dim">{pdu.opened.hidden}.</p>
              </section>
            ) : (
              <section className="pki-plaintext" data-testid="pdu-plaintext">
                <h4>
                  Plaintext <span className="dim">as {pdu.opened.by} decrypted it with its own key</span>
                </h4>
                {pdu.opened.error ? <p className="warn">It did not open: {pdu.opened.error}</p> : null}
                {pdu.opened.layers.map((l, i) => (
                  <Layer key={`p${l.path}`} layer={l} index={100 + i} />
                ))}
              </section>
            )
          ) : null}
        </div>
      ) : null}
    </aside>
  );
}
