/**
 * Quick settings: the controls people change most, one compact row each, one click from the map.
 *
 * Every control is one the engine publishes (`scenario.get` `quick`, from
 * `crates/v2xw-engine/src/scenario/guide.rs`), under the heading it names: a field, whose choices
 * and named values are read off the field's own published row; a set of coherent choices (each a
 * patch the loader accepts); a fleet-mix toggle; or a share set on every element of a list. The
 * scenario and the city are picked in the header, not here. Edits go into the same draft the
 * settings window edits, so the two never disagree; "Apply and run" starts a run with them.
 */

import { useEffect, useMemo, useState } from "react";

import { engine } from "../state/engine.js";
import { currentFields, editSetting, runSettingsAction, runWithDraft } from "../state/settings.js";
import { useStudio, type QuickSetting } from "../state/store.js";
import { changedPointers, getPointer } from "../lib/schema.js";
import { applyPatch, classesWith, currentChoice, currentShare, describeSeconds, hasClass, listWithShare } from "./quick.js";
import type { Field } from "./model.js";

interface ExportedFile {
  readonly path: string;
  readonly rows?: number;
  readonly bytes?: number;
}

/** What the last finished run wrote (`run.status` `engine.exports`). */
function useExports(): { files: ExportedFile[]; error: string | null } {
  const state = useStudio((s) => s.run.state);
  const [out, setOut] = useState<{ files: ExportedFile[]; error: string | null }>({ files: [], error: null });
  useEffect(() => {
    if (state !== "finished") return;
    let live = true;
    void engine
      .request("run.status", {})
      .then((res) => {
        const ex = (res as unknown as { engine?: { exports?: unknown } }).engine?.exports;
        if (!live) return;
        if (ex && typeof ex === "object" && !Array.isArray(ex) && "error" in ex) {
          setOut({ files: [], error: String((ex as { error: unknown }).error) });
        } else if (Array.isArray(ex)) {
          setOut({ files: ex.flatMap((e: { files?: ExportedFile[] }) => e.files ?? []), error: null });
        } else setOut({ files: [], error: null });
      })
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, [state]);
  return out;
}

/** A share as a percentage: 0.05 → "5 %". */
function pct(x: number): string {
  return `${+(x * 100).toFixed(1)} %`;
}

function FieldControl({ q, field, value }: { q: QuickSetting; field: Field | undefined; value: unknown }): React.JSX.Element {
  const [custom, setCustom] = useState(false);
  if (!field) return <span className="faint">not in this engine</span>;
  const id = `quick-${q.id}`;
  if (field.choices && field.choices.length > 0) {
    const current = value === undefined || value === null ? "" : String(value);
    const def = field.hasDefault && field.defaultValue !== undefined && field.defaultValue !== null ? String(field.defaultValue) : null;
    return (
      <select
        id={id}
        value={current === "" && def !== null ? def : current}
        onChange={(e) => editSetting(field.pointer, field.kind === "number" || field.kind === "integer" ? Number(e.target.value) : e.target.value)}
        data-testid={id}
      >
        {current === "" && def === null ? <option value="">Engine default</option> : null}
        {field.choices.map((c) => (
          <option key={String(c.value)} value={String(c.value)} disabled={!c.available} title={c.description}>
            {c.label}
            {c.available ? "" : " (not available)"}
          </option>
        ))}
      </select>
    );
  }
  const presets = field.presets ?? [];
  const num = typeof value === "number" ? value : typeof field.defaultValue === "number" && value === undefined ? field.defaultValue : undefined;
  const at = presets.findIndex((p) => p.value === num);
  const isInt = field.kind === "integer";
  const showCustom = custom || (at < 0 && presets.length > 0);
  // A share (0..1) reads as a percentage; the document keeps the fraction.
  const isShare = field.unit === undefined && field.min === 0 && field.max === 1 && !isInt;
  return (
    <span className="quick-field">
      {presets.length > 0 ? (
        <select
          id={id}
          data-testid={id}
          value={showCustom ? "custom" : String(at)}
          onChange={(e) => {
            if (e.target.value === "custom") {
              setCustom(true);
              return;
            }
            setCustom(false);
            editSetting(field.pointer, presets[Number(e.target.value)].value);
          }}
        >
          {presets.map((p, i) => (
            <option key={p.label} value={String(i)} title={p.note}>
              {p.label}
            </option>
          ))}
          <option value="custom">Custom…</option>
        </select>
      ) : null}
      {showCustom || presets.length === 0 ? (
        <input
          type="number"
          aria-label={`${q.label}${field.unit ? ` in ${field.unit}` : isShare ? " in percent" : ""}`}
          data-testid={`${id}-custom`}
          value={num === undefined ? "" : isShare ? +(num * 100).toFixed(3) : num}
          min={isShare ? 0 : field.min}
          max={isShare ? 100 : field.max}
          step={isInt ? 1 : "any"}
          placeholder="default"
          onChange={(e) => {
            if (e.target.value === "") return;
            const n = Number(e.target.value);
            if (Number.isFinite(n)) editSetting(field.pointer, isShare ? n / 100 : isInt ? Math.round(n) : n);
          }}
        />
      ) : null}
      {field.unit ? <span className="unit">{field.unit}</span> : isShare ? <span className="unit">%</span> : null}
      {field.pointer === "/time/duration_s" && num !== undefined ? <span className="faint">{describeSeconds(num)}</span> : null}
    </span>
  );
}

export function QuickSettings({ onClose, onOpenSettings }: { onClose: () => void; onOpenSettings: () => void }): React.JSX.Element {
  const quick = useStudio((s) => s.scenarioExtras.quick);
  const draft = useStudio((s) => s.draft);
  const scenario = useStudio((s) => s.scenario);
  const busy = useStudio((s) => s.settingsBusy);
  const message = useStudio((s) => s.settingsMessage);
  const fieldsSource = useStudio((s) => s.scenarioExtras.fields);
  const exports = useExports();

  useEffect(() => {
    const onKey = (e: KeyboardEvent): void => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const doc = draft ?? scenario;
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const fields = useMemo(() => currentFields(), [fieldsSource, scenario]);
  const byPointer = useMemo(() => new Map(fields.map((f) => [f.pointer, f])), [fields]);
  const changed = useMemo(() => (draft === null ? [] : changedPointers(scenario, draft)), [draft, scenario]);
  const edits = changed.length;
  // The scenario and the city are the header's; an older engine that still lists them here is
  // not repeated.
  const shown = quick.filter((q) => q.kind !== "scenario" && q.kind !== "city");

  const control = (q: QuickSetting): React.JSX.Element | null => {
    if (q.kind === "field" && q.pointer) {
      return <FieldControl q={q} field={byPointer.get(q.pointer)} value={getPointer(doc, q.pointer)} />;
    }
    if (q.kind === "class" && q.pointer && q.class) {
      const classes = getPointer(doc, q.pointer);
      const on = hasClass(classes, q.class);
      const share = on ? (classes as Record<string, { fraction?: number }>)[q.class]?.fraction : undefined;
      return (
        <label className="quick-toggle">
          <input
            type="checkbox"
            data-testid={`quick-${q.id}`}
            checked={on}
            onChange={(e) => editSetting(q.pointer!, classesWith(classes, q.class!, q.share ?? 0, q.absorb ?? "passenger", e.target.checked))}
          />
          <span className="dim">{on ? (typeof share === "number" ? pct(share) : "on") : "off"}</span>
        </label>
      );
    }
    if (q.kind === "share" && q.pointer && q.key && q.values) {
      const list = getPointer(doc, q.pointer);
      const empty = !Array.isArray(list) || list.length === 0;
      const now = currentShare(list, q.key);
      const values = now !== null && !q.values.includes(now) ? [...q.values, now].sort((a, b) => a - b) : q.values;
      return (
        <select
          data-testid={`quick-${q.id}`}
          disabled={empty}
          title={empty ? "Choose an attacker first" : undefined}
          value={now === null ? "" : String(now)}
          onChange={(e) => editSetting(q.pointer!, listWithShare(getPointer(useStudio.getState().draft ?? useStudio.getState().scenario, q.pointer!), q.key!, Number(e.target.value)))}
        >
          {now === null ? <option value="">{empty ? "None" : "Mixed"}</option> : null}
          {values.map((v) => (
            <option key={v} value={String(v)}>
              {pct(v)}
            </option>
          ))}
        </select>
      );
    }
    if (q.kind === "choice" && q.options && q.options.length > 0) {
      const opts = q.options;
      const at = currentChoice(doc, opts);
      const groups: string[] = [];
      for (const o of opts) if (o.group && !groups.includes(o.group)) groups.push(o.group);
      const option = (i: number): React.JSX.Element => (
        <option key={i} value={String(i)} title={opts[i].description}>
          {opts[i].label}
        </option>
      );
      return (
        <select
          data-testid={`quick-${q.id}`}
          value={at < 0 ? "" : String(at)}
          title={at >= 0 ? opts[at].description : undefined}
          onChange={(e) => {
            const o = opts[Number(e.target.value)];
            if (!o) return;
            const next = applyPatch(useStudio.getState().draft ?? useStudio.getState().scenario, o.patch);
            for (const w of o.patch) editSetting(w.pointer, getPointer(next, w.pointer));
          }}
        >
          {at < 0 ? <option value="">Custom</option> : null}
          {groups.length > 0
            ? groups.map((g) => (
                <optgroup key={g} label={g}>
                  {opts.map((o, i) => (o.group === g ? option(i) : null))}
                </optgroup>
              ))
            : opts.map((_, i) => option(i))}
        </select>
      );
    }
    return null;
  };

  const touches = (q: QuickSetting): boolean => {
    const ptrs = q.pointer ? [q.pointer] : (q.options ?? []).flatMap((o) => o.patch.map((w) => w.pointer));
    return changed.some((c) => ptrs.some((p) => c === p || c.startsWith(`${p}/`)));
  };

  const rows: React.JSX.Element[] = [];
  let lastGroup: string | undefined;
  for (const q of shown) {
    const c = control(q);
    if (c === null) continue;
    if (q.group && q.group !== lastGroup) {
      rows.push(
        <div key={`g-${q.group}`} className="quick-group">
          {q.group}
        </div>,
      );
      lastGroup = q.group;
    }
    rows.push(
      <div key={q.id} className={touches(q) ? "quick-row changed" : "quick-row"} data-testid="quick-row" data-quick={q.id} title={q.explain}>
        <label className="quick-label">{q.label}</label>
        <div className="quick-control">{c}</div>
      </div>,
    );
    if (q.shows_files && (exports.files.length > 0 || exports.error)) {
      rows.push(
        <div key={`${q.id}-files`} className="quick-files" data-testid="quick-files">
          {exports.error ? (
            <span className="err-text">Not written: {exports.error}</span>
          ) : (
            <span className="dim" title={exports.files.map((f) => f.path).join("\n")}>
              Last run wrote {exports.files.length} file{exports.files.length === 1 ? "" : "s"}
              {exports.files[0] ? (
                <>
                  {" "}
                  in <code>{exports.files[0].path.replace(/[^/]*$/, "")}</code>
                </>
              ) : null}
            </span>
          )}
        </div>,
      );
    }
  }

  return (
    <div className="quick-settings" role="dialog" aria-label="Quick settings" data-testid="quick-settings">
      <div className="quick-head">
        <b>Quick settings</b>
        <span className="grow" />
        <button type="button" className="icon-button" onClick={onClose} aria-label="Close" title="Close (Esc)">
          ×
        </button>
      </div>
      <div className="quick-body">{rows.length === 0 ? <p className="dim">This engine publishes no quick settings. Every setting is in Settings.</p> : rows}</div>
      {message ? <div className={`quick-message ${message.tone === "err" ? "err-text" : "dim"}`}>{message.text}</div> : null}
      <div className="quick-foot">
        <span className="dim grow">{edits > 0 ? `${edits} change${edits === 1 ? "" : "s"}` : "No changes"}</span>
        <button type="button" onClick={onOpenSettings} data-testid="quick-all-settings" title="Every setting, by section">
          All settings
        </button>
        <button
          type="button"
          className="primary"
          disabled={busy !== null}
          data-testid="quick-run"
          onClick={() =>
            void runSettingsAction("Run", async () => {
              const said = await runWithDraft();
              onClose();
              return said;
            })
          }
        >
          {busy === "Run" ? "Starting…" : edits > 0 ? "Apply and run" : "Run"}
        </button>
      </div>
    </div>
  );
}
