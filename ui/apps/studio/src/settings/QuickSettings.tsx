/**
 * Quick settings: the dozen choices that decide what a run models, one click from the map.
 *
 * Every control is one the engine publishes (`scenario.get` `quick`, from
 * `crates/v2xw-engine/src/scenario/guide.rs`): a field, whose choices and named values are read
 * off the field's own published row, a set of coherent choices (each a patch the loader accepts),
 * a fleet-mix toggle, or the scenario and the city. Edits go into the same draft the settings
 * window edits, so the two never disagree; "Apply and run" starts a run with them.
 */

import { useEffect, useMemo, useState } from "react";

import { engine } from "../state/engine.js";
import { cityLabel, refreshCities, useCities } from "../state/cities.js";
import { currentFields, editSetting, presetsOf, runSettingsAction, runWithDraft } from "../state/settings.js";
import { useStudio, type QuickSetting } from "../state/store.js";
import { changedPointers, getPointer } from "../lib/schema.js";
import { describeError } from "../lib/errors.js";
import { applyPatch, classesWith, currentChoice, describeSeconds, hasClass } from "./quick.js";
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

function FieldControl({ q, field, value }: { q: QuickSetting; field: Field | undefined; value: unknown }): React.JSX.Element {
  const [custom, setCustom] = useState(false);
  if (!field) return <span className="faint">not published by this engine</span>;
  const id = `quick-${q.id}`;
  if (field.choices && field.choices.length > 0) {
    return (
      <select id={id} value={value === undefined || value === null ? "" : String(value)} onChange={(e) => editSetting(field.pointer, field.kind === "number" || field.kind === "integer" ? Number(e.target.value) : e.target.value)} data-testid={id}>
        {value === undefined || value === null ? <option value="">Engine default</option> : null}
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
  const num = typeof value === "number" ? value : undefined;
  const at = presets.findIndex((p) => p.value === num);
  const isInt = field.kind === "integer";
  const showCustom = custom || (at < 0 && presets.length > 0);
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
          aria-label={`${q.label}${field.unit ? ` in ${field.unit}` : ""}`}
          data-testid={`${id}-custom`}
          value={num ?? ""}
          min={field.min}
          max={field.max}
          step={isInt ? 1 : "any"}
          placeholder="default"
          onChange={(e) => {
            if (e.target.value === "") return;
            const n = Number(e.target.value);
            if (Number.isFinite(n)) editSetting(field.pointer, isInt ? Math.round(n) : n);
          }}
        />
      ) : null}
      {field.unit ? <span className="unit">{field.unit}</span> : null}
      {field.pointer === "/time/duration_s" && num !== undefined ? <span className="faint">{describeSeconds(num)}</span> : null}
    </span>
  );
}

export function QuickSettings({ onClose, onOpenSettings }: { onClose: () => void; onOpenSettings: () => void }): React.JSX.Element {
  const quick = useStudio((s) => s.scenarioExtras.quick);
  const draft = useStudio((s) => s.draft);
  const scenario = useStudio((s) => s.scenario);
  const list = useStudio((s) => s.scenarioList);
  const runningHash = useStudio((s) => s.scenarioExtras.runningHash);
  const busy = useStudio((s) => s.settingsBusy);
  const message = useStudio((s) => s.settingsMessage);
  const fieldsSource = useStudio((s) => s.scenarioExtras.fields);
  const cities = useCities((s) => s.cities);
  const currentCity = useCities((s) => s.current);
  const presets = presetsOf(list);
  const [scenarioId, setScenarioId] = useState<string | null>(() => presets.find((p) => p.hash === runningHash)?.id ?? null);
  const [loadNote, setLoadNote] = useState<{ text: string; err: boolean } | null>(null);
  const exports = useExports();

  useEffect(() => {
    void refreshCities();
  }, []);
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
  const edits = useMemo(() => (draft === null ? 0 : changedPointers(scenario, draft).length), [draft, scenario]);

  const load = (path: string, city: string | null): void => {
    setLoadNote(null);
    const params: { path: string; city?: string; validate: boolean } = { path, validate: true };
    if (city) params.city = city;
    void engine
      .request("scenario.load", params)
      .then(async (res) => {
        useStudio.getState().setDraft(null);
        await engine.refreshScenario();
        await refreshCities();
        if (!res.valid) {
          useStudio.getState().setValidation({ valid: false, errors: res.errors ?? [], warnings: [] });
          setLoadNote({ text: "That combination does not load; each problem is marked in Settings.", err: true });
        } else setLoadNote({ text: "Loaded. Its own settings replace any unapplied quick-setting edits.", err: false });
      })
      .catch((err: unknown) => setLoadNote({ text: describeError(err), err: true }));
  };

  const row = (q: QuickSetting): React.JSX.Element | null => {
    let control: React.JSX.Element;
    if (q.kind === "scenario") {
      control = (
        <select
          data-testid="quick-scenario"
          value={scenarioId ?? ""}
          onChange={(e) => {
            setScenarioId(e.target.value);
            load(e.target.value, currentCity);
          }}
        >
          {scenarioId === null ? <option value="">The scenario on screen</option> : null}
          {presets.map((p) => (
            <option key={p.id} value={p.id} title={p.description}>
              {p.name ?? p.id}
            </option>
          ))}
        </select>
      );
    } else if (q.kind === "city") {
      const present = cities.filter((c) => c.present);
      control = (
        <select
          data-testid="quick-city"
          value={currentCity ?? ""}
          disabled={scenarioId === null}
          title={scenarioId === null ? "Choose a scenario first: the city is where that scenario runs" : undefined}
          onChange={(e) => scenarioId !== null && load(scenarioId, e.target.value || null)}
        >
          <option value="">The scenario's own map</option>
          {present.map((c) => (
            <option key={c.id} value={c.id}>
              {cityLabel(c)}
            </option>
          ))}
        </select>
      );
    } else if (q.kind === "field" && q.pointer) {
      control = <FieldControl q={q} field={byPointer.get(q.pointer)} value={getPointer(doc, q.pointer)} />;
    } else if (q.kind === "class" && q.pointer && q.class) {
      const classes = getPointer(doc, q.pointer);
      const on = hasClass(classes, q.class);
      control = (
        <label className="quick-toggle">
          <input
            type="checkbox"
            data-testid={`quick-${q.id}`}
            checked={on}
            onChange={(e) => editSetting(q.pointer!, classesWith(classes, q.class!, q.share ?? 0, q.absorb ?? "passenger", e.target.checked))}
          />
          <span className="dim">{on ? "on" : "off"}</span>
        </label>
      );
    } else if (q.kind === "choice" && q.options && q.options.length > 0) {
      const opts = q.options;
      const at = currentChoice(doc, opts);
      const groups: string[] = [];
      for (const o of opts) if (o.group && !groups.includes(o.group)) groups.push(o.group);
      const option = (i: number): React.JSX.Element => (
        <option key={i} value={String(i)} title={opts[i].description}>
          {opts[i].label}
        </option>
      );
      control = (
        <select
          data-testid={`quick-${q.id}`}
          value={at < 0 ? "" : String(at)}
          onChange={(e) => {
            const o = opts[Number(e.target.value)];
            if (!o) return;
            const next = applyPatch(useStudio.getState().draft ?? useStudio.getState().scenario, o.patch);
            for (const w of o.patch) editSetting(w.pointer, getPointer(next, w.pointer));
          }}
        >
          {at < 0 ? <option value="">Custom (set in Settings)</option> : null}
          {groups.length > 0
            ? groups.map((g) => (
                <optgroup key={g} label={g}>
                  {opts.map((o, i) => (o.group === g ? option(i) : null))}
                </optgroup>
              ))
            : opts.map((_, i) => option(i))}
        </select>
      );
    } else return null;
    return (
      <div key={q.id} className="quick-row" data-testid="quick-row" data-quick={q.id}>
        <label className="quick-label" title={q.explain}>
          {q.label}
        </label>
        <div className="quick-control">{control}</div>
        {q.explain ? <div className="quick-explain faint">{q.explain}</div> : null}
        {q.shows_files && (exports.files.length > 0 || exports.error) ? (
          <div className="quick-files" data-testid="quick-files">
            {exports.error ? (
              <span className="err-text">The last run's files were not written: {exports.error}</span>
            ) : (
              <>
                <span className="dim">The last run wrote:</span>
                <ul>
                  {exports.files.slice(0, 8).map((f) => (
                    <li key={f.path}>
                      <code>{f.path}</code>
                    </li>
                  ))}
                </ul>
                {exports.files.length > 8 ? <span className="faint">and {exports.files.length - 8} more in the same folder.</span> : null}
              </>
            )}
          </div>
        ) : null}
      </div>
    );
  };

  return (
    <div className="quick-settings" role="dialog" aria-label="Quick settings" data-testid="quick-settings">
      <div className="quick-head">
        <b>Quick settings</b>
        <span className="grow" />
        <button type="button" className="icon-button" onClick={onClose} aria-label="Close" title="Close (Esc)">
          ×
        </button>
      </div>
      <div className="quick-body">
        {quick.length === 0 ? <p className="dim">This engine publishes no quick settings. Every setting is in Settings.</p> : quick.map(row)}
        {loadNote ? <div className={loadNote.err ? "err-text" : "dim"}>{loadNote.text}</div> : null}
      </div>
      <div className="quick-foot">
        <span className="dim grow">{edits > 0 ? `${edits} change${edits === 1 ? "" : "s"} for the next run` : "No changes yet"}</span>
        <button type="button" onClick={onOpenSettings} data-testid="quick-all-settings" title="Every setting, with what each does">
          All settings
        </button>
        <button
          type="button"
          className="primary"
          disabled={busy !== null}
          data-testid="quick-run"
          onClick={() => void runSettingsAction("Run", async () => {
            const said = await runWithDraft();
            onClose();
            return said;
          })}
        >
          {busy === "Run" ? "Starting…" : edits > 0 ? "Apply and run" : "Run"}
        </button>
      </div>
      {message ? <div className={`quick-message ${message.tone === "err" ? "err-text" : "dim"}`}>{message.text}</div> : null}
    </div>
  );
}
