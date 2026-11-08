/**
 * Where a number comes from: a small note, opened from the few places that offer a "source" link
 * (a metric's "Which model produced it", an overlay's "why"), closed with its x or Escape.
 *
 * This replaces the inspector's Why tab, which the owner found meaningless (2026-10-06): a whole tab
 * every HUD value opened, which for most of them could only say "the stream does not say". What is
 * kept is the part that answers something:
 *
 *  1. If the value arrived with a model reference — measurement samples do — the model, its version
 *     and its parameter set, resolved from what the engine already sent.
 *  2. If the browser computed it (an overlay, a difference between two runs), what computed it and
 *     from what; there is no model, and the note does not pretend there is.
 *  3. Otherwise one button that asks the engine (`explain`).
 */

import { useCallback, useEffect, useState } from "react";
import type { ExplainResult } from "@vwp/protocol";

import { CloseIcon } from "../shell/Icons.js";
import { engine } from "../state/engine.js";
import { useStudio } from "../state/store.js";

export function SourceNote(): React.JSX.Element | null {
  const why = useStudio((s) => s.why);
  const setWhy = useStudio((s) => s.setWhy);
  const metricProvenance = useStudio((s) => s.metricProvenance);
  const [explain, setExplain] = useState<ExplainResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    setExplain(null);
    setError(null);
  }, [why?.id, why?.node, why?.kind]);

  useEffect(() => {
    if (!why) return;
    const onKey = (ev: KeyboardEvent): void => {
      if (ev.key === "Escape" && !ev.defaultPrevented) {
        setWhy(null);
        ev.preventDefault();
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [why, setWhy]);

  const ask = useCallback(async () => {
    if (!why) return;
    setBusy(true);
    setError(null);
    try {
      const res = await engine.request("explain", {
        subject: {
          kind: why.kind,
          id: why.id,
          ...(why.node !== undefined ? { node: why.node } : {}),
          ...(why.actor !== undefined ? { actor: why.actor } : {}),
          ...(why.provId !== undefined ? { prov_id: why.provId } : {}),
        },
        depth: 2,
        format: "json",
      });
      setExplain(res);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }, [why]);

  if (!why) return null;
  const wireProvId = why.provId ?? (why.kind === "metric" ? metricProvenance[why.id] : undefined);
  const local = wireProvId !== undefined ? engine.resolveProvenance(wireProvId) : null;
  const model = local
    ? { id: local.modelId, version: local.modelVersion, params: local.paramSetId, card: local.cardUrl }
    : explain && explain.chain.length > 0
      ? { id: explain.chain[0].model_id, version: explain.chain[0].model_version, params: explain.chain[0].param_set_id, card: explain.chain[0].card_url ?? "" }
      : null;

  return (
    <aside className="source-note" role="dialog" aria-label={`Source of ${why.label}`} data-testid="source-note">
      <div className="source-note-head">
        <b data-testid="source-note-value">
          {why.label}
          {why.value !== undefined ? ` = ${why.value}` : ""}
        </b>
        <button type="button" className="icon-button" onClick={() => setWhy(null)} aria-label="Close" title="Close (Esc)">
          <CloseIcon />
        </button>
      </div>
      {why.client ? (
        <dl className="kv" data-testid="source-note-client">
          <dt>computed by</dt>
          <dd>{why.client.producer}</dd>
          <dt>how</dt>
          <dd>{why.client.computation}</dd>
          <dt>from</dt>
          <dd>{why.client.inputs}</dd>
          {why.client.reference ? (
            <>
              <dt>rule</dt>
              <dd>{why.client.reference}</dd>
            </>
          ) : null}
          {why.client.caveat ? (
            <>
              <dt>caveat</dt>
              <dd>{why.client.caveat}</dd>
            </>
          ) : null}
        </dl>
      ) : null}
      {model ? (
        <dl className="kv" data-testid="source-note-model">
          <dt>model</dt>
          <dd>
            {model.id || "—"} {model.version ? <span className="faint">v{model.version}</span> : null}
          </dd>
          <dt>parameters</dt>
          <dd>{model.params || "—"}</dd>
          {model.card ? (
            <>
              <dt>model card</dt>
              <dd>
                <a href={model.card} target="_blank" rel="noreferrer">
                  open
                </a>
              </dd>
            </>
          ) : null}
        </dl>
      ) : why.client === undefined || wireProvId !== undefined ? (
        <button type="button" className="small" onClick={() => void ask()} disabled={busy} data-testid="source-note-ask">
          {busy ? "Asking…" : "Ask the engine which model produced it"}
        </button>
      ) : null}
      {explain?.definition_md ? <p className="dim">{explain.definition_md}</p> : null}
      {error ? <p className="err-text">The engine could not say: {error}</p> : null}
    </aside>
  );
}
