/**
 * The settings window's controls for a value from a known set, and for a number with named values.
 *
 * The owner's review (2026-10-06): "a lot of fields the user needs to manually fill out … the user
 * doesn't really know what is supposed to go there". Every set of values the engine knows is
 * published with words (`x-options`: a label and one sentence per value) and every number that has
 * well-known values with its presets (`x-presets`); these controls show them. Nothing here names a
 * model id: the lists come from the engine, so a model added to the engine appears here unedited.
 */

import { useContext, useEffect, useState } from "react";

import type { Choice, ItemField, NumberPreset, ScalarKind } from "../lib/schema.js";
import { getPointer, joinComposed, presetIn, providersIn, setPointer, splitComposed } from "../lib/schema.js";
import { SettingsDocContext } from "./docContext.js";
import { durationWords } from "../lib/duration.js";
import { engine } from "../state/engine.js";
import { useStudio } from "../state/store.js";
import { PARAM_SPECS, type ParamSpec } from "./paramSpecs.js";

import "../styles/settings-widgets.css";

interface Common {
  readonly id: string;
  readonly "aria-invalid"?: boolean;
  readonly "aria-describedby": string;
}

const CUSTOM = "\u0000custom";
const UNSET = "\u0000unset";

function sameValue(a: unknown, b: Choice["value"]): boolean {
  return a === b || (a !== undefined && a !== null && String(a) === String(b));
}

/**
 * A dropdown over a published set, with each option's sentence under it once chosen.
 *
 * * A value the document holds that the set does not list is kept and shown, never silently
 *   replaced.
 * * An open set (a model slot the registry may not list in full) has "Other…", which turns the
 *   control into a text box.
 * * An optional field has "Engine default", which removes the key.
 * * A value the engine names and refuses is listed, disabled, with its reason in the tooltip.
 */
export function ChoiceSelect({
  common,
  choices,
  open,
  nullable,
  value,
  onChange,
  compact,
  defaultValue,
  unsetLabel,
}: {
  common: Common;
  choices: readonly Choice[];
  open: boolean;
  nullable: boolean;
  value: unknown;
  onChange: (v: unknown) => void;
  compact?: boolean;
  /** The value the engine uses while the key is unset: the unset entry names it. */
  defaultValue?: unknown;
  /** The words for "unset" when unset is a choice of its own ("None"). */
  unsetLabel?: string;
}): React.JSX.Element {
  const known = choices.find((c) => sameValue(value, c.value));
  const fallback = defaultValue === undefined ? undefined : choices.find((c) => sameValue(defaultValue, c.value));
  const unset = value === undefined || value === null || value === "";
  const [typing, setTyping] = useState(false);
  useEffect(() => {
    if (known) setTyping(false);
  }, [known]);
  if (typing) {
    return (
      <span className="choice-custom">
        <input
          {...common}
          type="text"
          autoFocus
          value={unset ? "" : String(value)}
          placeholder="Type a model id"
          onChange={(e) => onChange(e.target.value === "" ? undefined : e.target.value)}
        />
        <button type="button" className="linkbutton" onClick={() => setTyping(false)}>
          Back to the list
        </button>
      </span>
    );
  }
  const current = known ? String(known.value) : unset ? UNSET : String(value);
  const shown = known ?? (unset ? fallback : undefined);
  return (
    <span className={compact ? "choice compact" : "choice"}>
      <span className="choice-row">
      <select
        {...common}
        value={current}
        data-testid="choice-select"
        onChange={(e) => {
          const v = e.target.value;
          if (v === CUSTOM) {
            setTyping(true);
            return;
          }
          if (v === UNSET) {
            onChange(undefined);
            return;
          }
          const picked = choices.find((c) => String(c.value) === v);
          onChange(picked ? picked.value : v);
        }}
      >
        {unset || nullable || unsetLabel ? (
          <option value={UNSET}>{unsetLabel ?? (fallback ? `${fallback.label} (default)` : "Default")}</option>
        ) : null}
        {!known && !unset ? <option value={String(value)}>{String(value)} (not in the list)</option> : null}
        {choices.map((c) => (
          <option key={String(c.value)} value={String(c.value)} disabled={!c.available} title={c.description}>
            {c.available ? c.label : `${c.label} — not available`}
            {c.us === false ? " (not US)" : ""}
          </option>
        ))}
        {open ? <option value={CUSTOM}>Other…</option> : null}
      </select>
      {compact ? null : <OptionExplainer choices={choices} />}
      </span>
      {!compact && shown?.description ? (
        <span className="choice-desc" data-testid="choice-desc">
          {shown.description}
        </span>
      ) : null}
    </span>
  );
}

/**
 * An "i" button beside a menu that lists every option with its sentence, its US badge and, for a
 * disabled option, the reason. A disabled `<option title>`
 * tooltip is not shown by Chrome or Safari on macOS, so the reason needs a place of its own, and a
 * reader should see what each value does before choosing one.
 */
export function OptionExplainer({ choices }: { choices: readonly Choice[] }): React.JSX.Element | null {
  const [open, setOpen] = useState(false);
  if (choices.length === 0) return null;
  return (
    <span className={open ? "option-explainer open" : "option-explainer"}>
      <button
        type="button"
        className="icon-button tiny"
        aria-expanded={open}
        aria-label="What each option does"
        title="What each option does"
        data-testid="option-explainer"
        onClick={() => setOpen((o) => !o)}
      >
        i
      </button>
      {open ? (
        <ul className="option-list" data-testid="option-list">
          {choices.map((c) => (
            <li key={String(c.value)} className={c.available ? "" : "off"}>
              <b>{c.label}</b>
              {c.us === true ? <span className="badge-us"> US</span> : null}
              {c.us === false ? <span className="badge-not-us"> not US</span> : null}
              {!c.available ? <span className="faint"> (not available)</span> : null}
              {c.description ? <span className="faint"> — {c.description}</span> : null}
            </li>
          ))}
        </ul>
      ) : null}
    </span>
  );
}

/** `[from, to]` as two number boxes (`x-pair`); clearing both removes the key. */
export function RangePair({
  common,
  value,
  min,
  max,
  unit,
  onChange,
}: {
  common: Common;
  value: unknown;
  min?: number;
  max?: number;
  unit?: string;
  onChange: (v: unknown) => void;
}): React.JSX.Element {
  const pair = Array.isArray(value) ? (value as unknown[]) : [];
  const at = (i: number): number | "" => (typeof pair[i] === "number" ? (pair[i] as number) : "");
  const set = (i: number, raw: string): void => {
    const next: (number | undefined)[] = [typeof pair[0] === "number" ? (pair[0] as number) : undefined, typeof pair[1] === "number" ? (pair[1] as number) : undefined];
    next[i] = raw === "" ? undefined : Number(raw);
    if (next[0] === undefined && next[1] === undefined) onChange(undefined);
    else onChange([next[0] ?? next[1] ?? 0, next[1] ?? next[0] ?? 0]);
  };
  return (
    <span className="range-pair" id={common.id} aria-describedby={common["aria-describedby"]} role="group">
      <label>
        from <input type="number" data-testid="pair-from" value={at(0)} min={min} max={max} step="any" onChange={(e) => set(0, e.target.value)} />
      </label>
      <label>
        to <input type="number" data-testid="pair-to" value={at(1)} min={min} max={max} step="any" onChange={(e) => set(1, e.target.value)} />
      </label>
      {unit ? <span className="unit">{unit}</span> : null}
    </span>
  );
}

/**
 * A map edited one row per key: the key is a menu of the keys
 * the engine publishes (`x-key-options`), the value its own control, or a row of fields when the
 * value is a struct. A key already used is not offered again.
 */
export function MapRows({
  idBase,
  keyChoices,
  keyOpen,
  valueFields,
  value,
  onChange,
}: {
  idBase: string;
  keyChoices: readonly Choice[];
  keyOpen: boolean;
  valueFields: readonly ItemField[];
  value: unknown;
  onChange: (v: unknown) => void;
}): React.JSX.Element {
  const map = value !== null && typeof value === "object" && !Array.isArray(value) ? (value as Record<string, unknown>) : {};
  const keys = Object.keys(map);
  const leaf = valueFields.length === 1 && valueFields[0].key === "" ? valueFields[0] : undefined;
  const emit = (next: Record<string, unknown>): void => onChange(Object.keys(next).length === 0 ? undefined : next);
  const rename = (from: string, to: string): void => {
    if (to === from || to === "" || to in map) return;
    const next: Record<string, unknown> = {};
    for (const k of keys) next[k === from ? to : k] = map[k];
    emit(next);
  };
  const free = keyChoices.filter((c) => c.available && !(String(c.value) in map));
  const emptyValue = (): unknown => (leaf ? (leaf.kind === "number" || leaf.kind === "integer" ? (leaf.min ?? 0) : leaf.choices?.find((c) => c.available)?.value ?? "") : {});
  return (
    <div className="map-rows" data-testid="map-rows">
      {keys.length === 0 ? <div className="dim empty-note">None</div> : null}
      {keys.map((k) => {
        const id = `${idBase}-${k.replace(/\W/g, "_")}`;
        const own = keyChoices.find((c) => String(c.value) === k);
        return (
          <div className="map-row" key={k} data-testid="map-row">
            <ChoiceSelect
              common={{ id: `${id}-key`, "aria-describedby": `${id}-key` }}
              choices={own ? [own, ...free] : free}
              open={keyOpen}
              nullable={false}
              value={k}
              onChange={(v) => rename(k, String(v ?? ""))}
              compact
            />
            {leaf ? (
              <ItemControl id={`${id}-v`} field={leaf} value={map[k]} onChange={(v) => emit({ ...map, [k]: v })} />
            ) : (
              <div className="list-item-grid">
                {valueFields.map((f) => {
                  const fid = `${id}-${f.key.replace(/\W/g, "_")}`;
                  return (
                    <label key={f.key} className="list-cell" htmlFor={fid} title={f.help}>
                      <span className="list-cell-label">
                        {f.label}
                        {f.unit ? <span className="unit"> [{f.unit}]</span> : null}
                      </span>
                      <ItemControl id={fid} field={f} value={getPointer(map[k], f.key)} onChange={(v) => emit({ ...map, [k]: prune(setPointer(map[k] ?? {}, f.key, v)) })} />
                    </label>
                  );
                })}
              </div>
            )}
            <button
              type="button"
              className="linkbutton"
              data-testid="map-remove"
              onClick={() => {
                const next = { ...map };
                delete next[k];
                emit(next);
              }}
            >
              Remove
            </button>
          </div>
        );
      })}
      {free.length > 0 || keyOpen ? (
        <button
          type="button"
          className="btn small"
          data-testid="map-add"
          onClick={() => {
            const k = free[0] ? String(free[0].value) : `new-${keys.length + 1}`;
            emit({ ...map, [k]: emptyValue() });
          }}
        >
          Add
        </button>
      ) : null}
    </div>
  );
}

/** A number box with the field's named values as buttons beside it; the box keeps any number. */
export function NumberWithPresets({
  common,
  value,
  kind,
  min,
  max,
  step,
  presets,
  unit,
  onChange,
  defaultValue,
  emptyText,
}: {
  common: Common;
  value: unknown;
  kind: "number" | "integer";
  min?: number;
  max?: number;
  step?: number;
  presets?: readonly NumberPreset[];
  unit?: string;
  onChange: (v: unknown) => void;
  /** Shown greyed in the empty box: what the engine uses while the key is unset. */
  defaultValue?: unknown;
  /** What an empty box means when there is no number to show ("never"). */
  emptyText?: string;
}): React.JSX.Element {
  const input = (
    <input
      {...common}
      type="number"
      className="num"
      placeholder={typeof defaultValue === "number" ? String(defaultValue) : emptyText}
      value={typeof value === "number" ? value : ""}
      min={min}
      max={max}
      step={step ?? (kind === "integer" ? 1 : "any")}
      onChange={(e) => onChange(e.target.value === "" ? undefined : Number(e.target.value))}
    />
  );
  const effective = typeof value === "number" ? value : defaultValue;
  const words = unit === "s" && typeof effective === "number" ? durationWords(effective) : "";
  // The words are left out when the chip already says them ("10 min").
  const hideWords = words !== "" && (presets ?? []).some((p) => p.value === effective && p.label.toLowerCase().startsWith(words.toLowerCase()));
  const head = (
    <span className="num-box">
      {input}
      {words !== "" && !hideWords ? (
        <span className={typeof value === "number" ? "dim duration-words" : "faint duration-words"} data-testid="duration-words">
          {words}
        </span>
      ) : null}
    </span>
  );
  if (!presets || presets.length === 0) return <span className="number-presets">{head}</span>;
  const active = presets.find((p) => p.value === effective);
  return (
    <span className="number-presets">
      {head}
      <span className="preset-chips" role="group" aria-label="Common values">
        {presets.map((p) => (
          <button
            key={p.label}
            type="button"
            className={p === active ? (typeof value === "number" ? "preset-chip active" : "preset-chip active default") : "preset-chip"}
            aria-pressed={p === active}
            title={`${p.value}${unit ? ` ${unit}` : ""}${p.note ? ` — ${p.note}` : ""}`}
            data-testid="preset-chip"
            onClick={() => onChange(p.value)}
          >
            {p.label}
          </button>
        ))}
      </span>
    </span>
  );
}

/** A list of strings from a closed set, as checkboxes. Values outside the set stay, ticked. */
export function MultiCheck({
  common,
  choices,
  value,
  onChange,
}: {
  common: Common;
  choices: readonly Choice[];
  value: unknown;
  onChange: (v: unknown) => void;
}): React.JSX.Element {
  const list = Array.isArray(value) ? (value as unknown[]).map(String) : [];
  const extra = list.filter((v) => !choices.some((c) => String(c.value) === v));
  const toggle = (v: string, on: boolean): void => {
    // Keep the published order, so the document does not churn with the click order.
    const next = new Set(list);
    if (on) next.add(v);
    else next.delete(v);
    const ordered = [...choices.map((c) => String(c.value)).filter((x) => next.has(x)), ...extra.filter((x) => next.has(x))];
    onChange(ordered);
  };
  return (
    <div className="multi-check" id={common.id} aria-describedby={common["aria-describedby"]} role="group">
      {choices.map((c) => {
        const v = String(c.value);
        return (
          <label key={v} className={c.available ? "multi-item" : "multi-item off"} title={c.description}>
            <input type="checkbox" data-value={v} checked={list.includes(v)} disabled={!c.available && !list.includes(v)} onChange={(e) => toggle(v, e.target.checked)} />
            <span>
              <b>{c.label}</b>
              {c.description ? <span className="faint"> — {c.description}</span> : null}
            </span>
          </label>
        );
      })}
      {extra.map((v) => (
        <label key={v} className="multi-item">
          <input type="checkbox" data-value={v} checked onChange={(e) => toggle(v, e.target.checked)} />
          <span>
            <b>{v}</b> <span className="faint">(not in the engine's list)</span>
          </span>
        </label>
      ))}
    </div>
  );
}

/** One field of a list element, at its pointer inside the element. */
function ItemControl({
  id,
  field,
  value,
  onChange,
  item,
  listPointer,
}: {
  id: string;
  field: ItemField;
  value: unknown;
  onChange: (v: unknown) => void;
  /** The whole element, for a field whose control depends on a sibling (backhaul params on its backhaul). */
  item?: unknown;
  /** The list's own pointer, which names a parameter block's keys (`paramSpecs.ts`). */
  listPointer?: string;
}): React.JSX.Element {
  const common = { id, "aria-describedby": `${id}-desc` } as const;
  const spec = listPointer ? PARAM_SPECS[`${listPointer}/-${field.key}`] : undefined;
  if (spec) {
    const model = listPointer === "/radio/adjacent_channel" ? "adjacent" : getPointer(item, "/id");
    return <KeyedParams id={id} spec={spec} model={typeof model === "string" ? model : undefined} value={value} onChange={onChange} />;
  }
  if (field.kind === "multi" && field.choices) {
    return <MultiCheck common={common} choices={field.choices} value={value} onChange={(v) => onChange(Array.isArray(v) && v.length === 0 ? undefined : v)} />;
  }
  if (field.kind === "scalars") {
    return <ScalarList id={id} name={field.key} elemKind={field.elemKind ?? "string"} unit={field.unit} value={value} onChange={onChange} />;
  }
  if (field.key === "/backhaul_params") {
    return <BackhaulParams id={id} medium={backhaulOf(getPointer(item, "/backhaul"))} value={value} onChange={onChange} />;
  }
  if (field.compose && field.choices) {
    return <ProviderRolePicker id={id} field={field} compose={field.compose} value={value} onChange={onChange} />;
  }
  if (field.providerNames && field.choices) {
    return <ProviderName common={common} field={field} value={value} onChange={onChange} />;
  }
  if (field.kind === "map" && field.keyChoices) {
    return (
      <MapRows
        idBase={id}
        keyChoices={field.keyChoices}
        keyOpen={field.keyOpen === true}
        valueFields={field.valueFields ?? []}
        value={value}
        onChange={onChange}
      />
    );
  }
  if (field.choices) {
    return <ChoiceSelect common={common} choices={field.choices} open={field.choicesOpen === true} nullable={false} value={value} onChange={onChange} compact />;
  }
  if (field.kind === "boolean") {
    return <input {...common} type="checkbox" checked={value === true} onChange={(e) => onChange(e.target.checked)} />;
  }
  if ((field.kind === "number" || field.kind === "integer") && typeof value !== "string") {
    return (
      <input
        {...common}
        type="number"
        value={typeof value === "number" ? value : ""}
        min={field.min}
        max={field.max}
        step={field.step ?? (field.kind === "integer" ? 1 : "any")}
        placeholder="—"
        onChange={(e) => onChange(e.target.value === "" ? undefined : Number(e.target.value))}
      />
    );
  }
  if (field.kind === "json") return <ItemJson id={id} value={value} onChange={onChange} />;
  return <input {...common} type="text" value={value === undefined || value === null ? "" : String(value)} onChange={(e) => onChange(e.target.value === "" ? undefined : e.target.value)} />;
}

/**
 * A server of a named provider: the provider menu (the
 * deployment's providers, from its preset or its custom list) beside the role menu. The home
 * provider's server is written bare (`crlg`), another's qualified (`provider-b/crlg`).
 */
function ProviderRolePicker({
  id,
  field,
  compose,
  value,
  onChange,
}: {
  id: string;
  field: ItemField;
  compose: NonNullable<ItemField["compose"]>;
  value: unknown;
  onChange: (v: unknown) => void;
}): React.JSX.Element {
  const doc = useContext(SettingsDocContext);
  const providers = providersIn(compose.providers, doc);
  const { provider, role } = splitComposed(value, compose.separator, providers);
  const write = (p: string, r: string): void => onChange(joinComposed(p, r, compose.separator, providers));
  const providerChoices: Choice[] = providers.map((p, i) => ({
    value: p,
    label: i === 0 ? `${p} (home)` : p,
    description: i === 0 ? "The home provider: its servers are named bare." : "Another provider on the trust list.",
    available: true,
  }));
  return (
    <span className="compose-picker" data-testid="provider-role-picker">
      <ChoiceSelect
        common={{ id: `${id}-provider`, "aria-describedby": `${id}-desc` }}
        choices={providerChoices}
        open={false}
        nullable={false}
        value={provider}
        onChange={(p) => write(String(p ?? ""), role)}
        compact
      />
      <span className="faint">{compose.separator}</span>
      <ChoiceSelect
        common={{ id, "aria-describedby": `${id}-desc` }}
        choices={field.choices ?? []}
        open={field.choicesOpen === true}
        nullable={false}
        value={role}
        onChange={(r) => write(provider, String(r ?? ""))}
        compact
      />
    </span>
  );
}

/** A provider's name: the names the document's preset deploys; any name under a custom list. */
function ProviderName({
  common,
  field,
  value,
  onChange,
}: {
  common: Common;
  field: ItemField;
  value: unknown;
  onChange: (v: unknown) => void;
}): React.JSX.Element {
  const doc = useContext(SettingsDocContext);
  const sets = field.providerNames;
  const names = sets ? providersIn(sets, doc) : [];
  const custom = sets !== undefined && presetIn(sets, doc) === "custom";
  const choices = custom ? (field.choices ?? []) : (field.choices ?? []).filter((c) => names.includes(String(c.value)));
  return <ChoiceSelect common={common} choices={choices} open={custom || field.choicesOpen === true} nullable={false} value={value} onChange={onChange} compact />;
}

function ItemJson({ id, value, onChange }: { id: string; value: unknown; onChange: (v: unknown) => void }): React.JSX.Element {
  const serialised = value === undefined || value === null ? "" : JSON.stringify(value);
  const [text, setText] = useState(serialised);
  const [bad, setBad] = useState(false);
  useEffect(() => {
    setText(serialised);
    setBad(false);
  }, [serialised]);
  return (
    <input
      id={id}
      type="text"
      className={bad ? "bad mono" : "mono"}
      spellCheck={false}
      value={text}
      placeholder="{}"
      onChange={(e) => {
        setText(e.target.value);
        if (e.target.value.trim() === "") {
          setBad(false);
          onChange(undefined);
          return;
        }
        try {
          onChange(JSON.parse(e.target.value));
          setBad(false);
        } catch {
          setBad(true);
        }
      }}
    />
  );
}

/** Drop `undefined` members so a cleared field leaves the element rather than writing `null`. */
function prune(v: unknown): unknown {
  if (v === null || typeof v !== "object" || Array.isArray(v)) return v;
  const out: Record<string, unknown> = {};
  for (const [k, x] of Object.entries(v as Record<string, unknown>)) {
    if (x === undefined) continue;
    const p = prune(x);
    if (p !== null && typeof p === "object" && !Array.isArray(p) && Object.keys(p).length === 0) continue;
    out[k] = p;
  }
  return out;
}

/**
 * The members of an element no control covers (a position triple, a list of roles), as one small
 * JSON box, so nothing the element holds is hidden or lost.
 */
function OtherKeys({
  idBase,
  item,
  fields,
  onChange,
}: {
  idBase: string;
  item: unknown;
  fields: readonly ItemField[];
  onChange: (next: unknown) => void;
}): React.JSX.Element | null {
  if (item === null || typeof item !== "object" || Array.isArray(item)) return null;
  const covered = new Set(fields.map((f) => f.key.split("/")[1]));
  const obj = item as Record<string, unknown>;
  const rest = Object.fromEntries(Object.entries(obj).filter(([k]) => !covered.has(k)));
  if (Object.keys(rest).length === 0) return null;
  const id = `${idBase}-other`;
  return (
    <label className="list-cell wide" htmlFor={id}>
      <span className="list-cell-label">Other fields</span>
      <ItemJson
        id={id}
        value={rest}
        onChange={(v) => {
          const kept = Object.fromEntries(Object.entries(obj).filter(([k]) => covered.has(k)));
          onChange(v !== null && typeof v === "object" && !Array.isArray(v) ? { ...kept, ...(v as Record<string, unknown>) } : kept);
        }}
      />
    </label>
  );
}

/**
 * A list of objects, element by element: attackers, detectors, jammers. Each element's choice
 * (its model) is a dropdown and every other field a small control; "Add" appends the engine's
 * template for the list, which the loader accepts as it stands.
 */
export function ListEditor({
  idBase,
  fields,
  template,
  value,
  onChange,
  noun,
  pointer,
}: {
  /** The list's pointer, for the parameter blocks whose keys the window knows. */
  pointer?: string;
  idBase: string;
  fields: readonly ItemField[];
  template: unknown;
  value: unknown;
  onChange: (v: unknown) => void;
  noun: string;
}): React.JSX.Element {
  const list = Array.isArray(value) ? (value as unknown[]) : [];
  // The choice first (it is what the element *is*), then the rest in the engine's order; a set of
  // checkboxes (a unit's roles) last, where it has the room.
  const menu = (f: ItemField): boolean => f.choices !== undefined && f.kind !== "multi";
  const ordered = [...fields.filter(menu), ...fields.filter((f) => !menu(f) && f.kind !== "multi"), ...fields.filter((f) => f.kind === "multi")];
  const setItem = (i: number, key: string, v: unknown): void => {
    const next = list.slice();
    next[i] = prune(setPointer(next[i] ?? {}, key, v));
    onChange(next);
  };
  const fallbackTemplate = (): unknown => {
    const first = ordered[0];
    const v = first?.choices?.find((c) => c.available)?.value;
    return first && v !== undefined ? setPointer({}, first.key, v) : {};
  };
  return (
    <div className="list-editor" data-testid="list-editor">
      {list.length === 0 ? <div className="dim empty-note">None</div> : null}
      {list.map((item, i) => (
        <div className="list-item" key={i} data-testid="list-item">
          <div className="list-item-head">
            <span className="faint">
              {noun} {i + 1}
            </span>
            <span className="grow" />
            <button
              type="button"
              className="linkbutton"
              data-testid="list-remove"
              onClick={() => onChange(list.filter((_, j) => j !== i))}
            >
              Remove
            </button>
          </div>
          <div className="list-item-grid">
            {ordered.map((f) => {
              const id = `${idBase}-${i}-${f.key.replace(/\W/g, "_")}`;
              // A control of several boxes is not one label's: its cell is a plain block.
              const group =
                f.kind === "multi" || f.kind === "scalars" || f.key === "/backhaul_params" || (pointer !== undefined && PARAM_SPECS[`${pointer}/-${f.key}`] !== undefined);
              const Cell = group ? "div" : "label";
              return (
                <Cell
                  key={f.key}
                  className={f.choices || f.kind === "map" || group ? "list-cell wide" : "list-cell"}
                  {...(group ? {} : { htmlFor: id })}
                  title={f.help}
                  data-testid="list-cell"
                  data-key={f.key}
                >
                  <span className="list-cell-label">
                    {f.label}
                    {f.unit && !group ? <span className="unit"> [{f.unit}]</span> : null}
                  </span>
                  <ItemControl id={id} field={f} value={getPointer(item, f.key)} item={item} listPointer={pointer} onChange={(v) => setItem(i, f.key, v)} />
                </Cell>
              );
            })}
            <OtherKeys idBase={`${idBase}-${i}`} item={item} fields={ordered} onChange={(next) => {
              const copy = list.slice();
              copy[i] = next;
              onChange(copy);
            }} />
          </div>
        </div>
      ))}
      <button
        type="button"
        className="btn small"
        data-testid="list-add"
        onClick={() => onChange([...list, structuredClone(template ?? fallbackTemplate())])}
      >
        Add {noun.toLowerCase()}
      </button>
    </div>
  );
}

/** The value a list holds, as a list. */
function asList(value: unknown): unknown[] {
  return Array.isArray(value) ? (value as unknown[]) : [];
}

/**
 * A list of plain values. A position (`position_m`) is three labelled boxes, east, north and up,
 * with "Pick on map" when the window has a map; any other list is one text box of comma-separated
 * values ("a.rou.xml, b.rou.xml" or "0, 2"), each checked as it is typed.
 */
export function ScalarList({
  id,
  name,
  elemKind,
  unit,
  value,
  onChange,
}: {
  id: string;
  /** The key or pointer of the list: a position is recognised by its name. */
  name: string;
  elemKind: ScalarKind;
  unit?: string;
  value: unknown;
  onChange: (v: unknown) => void;
}): React.JSX.Element {
  if (elemKind !== "string" && /position(_m)?$/.test(name)) return <PositionBoxes id={id} unit={unit} value={value} onChange={onChange} />;
  return <CommaList id={id} elemKind={elemKind} unit={unit} value={value} onChange={onChange} />;
}

function CommaList({
  id,
  elemKind,
  unit,
  value,
  onChange,
}: {
  id: string;
  elemKind: ScalarKind;
  unit?: string;
  value: unknown;
  onChange: (v: unknown) => void;
}): React.JSX.Element {
  const serialised = asList(value).map(String).join(", ");
  const [text, setText] = useState(serialised);
  const [bad, setBad] = useState<string | null>(null);
  useEffect(() => {
    setText(serialised);
    setBad(null);
  }, [serialised]);
  const numeric = elemKind !== "string";
  return (
    <span className="scalar-list" data-testid="scalar-list">
      <span className="num-box">
        <input
          id={id}
          type="text"
          className={bad ? "bad" : undefined}
          value={text}
          placeholder={numeric ? "e.g. 0, 2" : "comma-separated"}
          spellCheck={false}
          onChange={(e) => {
            setText(e.target.value);
            const parts = e.target.value.split(",").map((t) => t.trim()).filter((t) => t !== "");
            if (parts.length === 0) {
              setBad(null);
              onChange(undefined);
              return;
            }
            if (!numeric) {
              setBad(null);
              onChange(parts);
              return;
            }
            const nums = parts.map(Number);
            const wrong = parts.find((_t, k) => !Number.isFinite(nums[k]) || (elemKind === "integer" && !Number.isInteger(nums[k])));
            if (wrong !== undefined) {
              setBad(`"${wrong}" is not ${elemKind === "integer" ? "a whole number" : "a number"}.`);
              return;
            }
            setBad(null);
            onChange(nums);
          }}
        />
        {unit ? <span className="unit">{unit}</span> : null}
      </span>
      <span className={bad ? "help err-text" : "faint"}>{bad ?? "Separate values with commas."}</span>
    </span>
  );
}

const AXES = ["east", "north", "up"] as const;

function PositionBoxes({ id, unit, value, onChange }: { id: string; unit?: string; value: unknown; onChange: (v: unknown) => void }): React.JSX.Element {
  const xyz = asList(value);
  const at = (k: number): number | "" => (typeof xyz[k] === "number" ? (xyz[k] as number) : "");
  const [note, setNote] = useState<string | null>(null);
  const write = (k: number, raw: string): void => {
    const next = [0, 1, 2].map((j) => (typeof xyz[j] === "number" ? (xyz[j] as number) : 0));
    next[k] = raw === "" ? 0 : Number(raw);
    onChange(next);
  };
  // The studio's map answers one "pick a point" request; a detached settings window has no map.
  const canPick = engine.viewer !== null && engine.viewer !== undefined;
  return (
    <span className="position-boxes" data-testid="position-boxes">
      <span className="position-row">
        {AXES.map((axis, k) => (
          <label key={axis} className="position-axis">
            <span className="faint">{axis}</span>
            <input id={k === 0 ? id : `${id}-${axis}`} type="number" step="any" value={at(k)} placeholder="0" onChange={(e) => write(k, e.target.value)} />
          </label>
        ))}
        {unit ? <span className="unit">{unit}</span> : null}
        {canPick ? (
          <button
            type="button"
            className="btn small"
            data-testid="position-pick"
            onClick={() => {
              setNote("Click the map where the unit stands.");
              useStudio.getState().setMapPick({
                purpose: "position",
                resolve: ({ x, y }) => {
                  const z = typeof xyz[2] === "number" ? (xyz[2] as number) : 0;
                  onChange([Math.round(x * 10) / 10, Math.round(y * 10) / 10, z]);
                  setNote(null);
                },
              });
            }}
          >
            Pick on map
          </button>
        ) : null}
      </span>
      <span className="faint">{note ?? "Metres east, north and up from the map's origin."}</span>
    </span>
  );
}

/**
 * The numbers a backhaul medium reads (`v2xw-engine` `backend.rs` `backhaul_from`): each one a box
 * with its unit, left empty for the medium's own value. Keys the medium does not read are kept and
 * listed, never dropped. Which keys each medium reads is the engine's; change them there, then here.
 */
const BACKHAUL_KEYS: Readonly<Record<string, readonly { key: string; label: string; unit: string }[]>> = (() => {
  const rate = { key: "rate_mbps", label: "Up", unit: "Mbit/s" };
  const cap = { key: "capacity_mbps", label: "Up", unit: "Mbit/s" };
  const down = { key: "down_mbps", label: "Down", unit: "Mbit/s" };
  const lat = { key: "latency_ms", label: "Latency", unit: "ms" };
  const loss = { key: "loss", label: "Packet loss", unit: "0 to 0.5" };
  const wired = [rate, down, lat, { key: "distance_km", label: "Distance", unit: "km" }, { key: "hops", label: "Router hops", unit: "" }, loss];
  return {
    "backhaul/fixed": [cap, down, lat, loss],
    "backhaul/fibre": wired,
    "backhaul/dsl": wired,
    "backhaul/microwave": wired,
    "backhaul/cellular": [cap, lat, loss],
    "backhaul/none": [],
  };
})();

/** A backhaul id, or the engine's own default (`backhaul/fixed`) when none is set. */
export function backhaulOf(v: unknown): string {
  return typeof v === "string" && v !== "" ? v : "backhaul/fixed";
}

export function BackhaulParams({ id, medium, value, onChange }: { id: string; medium: string; value: unknown; onChange: (v: unknown) => void }): React.JSX.Element {
  const obj = value !== null && typeof value === "object" && !Array.isArray(value) ? (value as Record<string, unknown>) : {};
  const keys = BACKHAUL_KEYS[medium];
  if (keys === undefined) return <ItemJson id={id} value={value} onChange={onChange} />;
  const write = (k: string, raw: string): void => {
    const next: Record<string, unknown> = { ...obj };
    if (raw === "") delete next[k];
    else next[k] = Number(raw);
    onChange(Object.keys(next).length === 0 ? undefined : next);
  };
  const unread = Object.keys(obj).filter((k) => !keys.some((x) => x.key === k));
  if (keys.length === 0 && unread.length === 0) return <span className="faint">No link: nothing to set.</span>;
  return (
    <span className="backhaul-params" data-testid="backhaul-params" data-medium={medium}>
      <span className="backhaul-grid">
        {keys.map((k, n) => (
          <label key={k.key} className="backhaul-cell">
            <span className="faint">
              {k.label}
              {k.unit ? ` (${k.unit})` : ""}
            </span>
            <input
              id={n === 0 ? id : `${id}-${k.key}`}
              type="number"
              step="any"
              min={0}
              value={typeof obj[k.key] === "number" ? (obj[k.key] as number) : ""}
              placeholder="medium's own"
              data-key={k.key}
              onChange={(e) => write(k.key, e.target.value)}
            />
          </label>
        ))}
      </span>
      {unread.length > 0 ? (
        <span className="help err-text">
          Not read by this backhaul: {unread.join(", ")}.{" "}
          <button type="button" className="linkbutton" onClick={() => onChange(Object.fromEntries(Object.entries(obj).filter(([k]) => !unread.includes(k))))}>
            Remove
          </button>
        </span>
      ) : (
        <span className="faint">Leave a box empty for the medium's own value.</span>
      )}
    </span>
  );
}

/**
 * A parameter block the engine checks key by key (`paramSpecs.ts`), one box per key, with the
 * model's own value when a box is left empty. Keys the table does not cover stay in a small JSON
 * box, so nothing the block holds is hidden or lost.
 */
export function KeyedParams({
  id,
  spec,
  model,
  value,
  onChange,
}: {
  id: string;
  spec: readonly ParamSpec[];
  model?: string;
  value: unknown;
  onChange: (v: unknown) => void;
}): React.JSX.Element {
  const obj = value !== null && typeof value === "object" && !Array.isArray(value) ? (value as Record<string, unknown>) : {};
  const shown = spec.filter((p) => !p.onlyFor || (model !== undefined && p.onlyFor.includes(model)) || obj[p.key] !== undefined);
  const known = new Set(spec.map((p) => p.key));
  const rest = Object.fromEntries(Object.entries(obj).filter(([k]) => !known.has(k)));
  const write = (k: string, v: unknown): void => {
    const next: Record<string, unknown> = { ...obj };
    if (v === undefined) delete next[k];
    else next[k] = v;
    onChange(Object.keys(next).length === 0 ? undefined : next);
  };
  return (
    <span className="keyed-params" data-testid="keyed-params">
      <span className="backhaul-grid">
        {shown.map((p, n) => {
          const pid = n === 0 ? id : `${id}-${p.key}`;
          if (p.kind === "position") {
            return (
              <span key={p.key} className="backhaul-cell wide">
                <span className="faint">{p.label}</span>
                <PositionBoxes id={pid} unit={p.unit} value={obj[p.key]} onChange={(v) => write(p.key, v)} />
              </span>
            );
          }
          if (p.kind === "choice") {
            return (
              <span key={p.key} className="backhaul-cell wide">
                <span className="faint">{p.label}</span>
                <ChoiceSelect common={{ id: pid, "aria-describedby": pid }} choices={p.choices ?? []} open={false} nullable value={obj[p.key]} onChange={(v) => write(p.key, v)} compact />
              </span>
            );
          }
          return (
            <label key={p.key} className="backhaul-cell">
              <span className="faint">
                {p.label}
                {p.unit ? ` (${p.unit})` : ""}
              </span>
              <input
                id={pid}
                type="number"
                step={p.kind === "integer" ? 1 : "any"}
                min={p.min}
                max={p.max}
                data-key={p.key}
                placeholder="model's own"
                value={typeof obj[p.key] === "number" ? (obj[p.key] as number) : ""}
                onChange={(e) => write(p.key, e.target.value === "" ? undefined : Number(e.target.value))}
              />
            </label>
          );
        })}
      </span>
      {Object.keys(rest).length > 0 ? (
        <span className="keyed-rest">
          <span className="faint">Other keys</span>
          <ItemJson id={`${id}-rest`} value={rest} onChange={(v) => onChange({ ...Object.fromEntries(Object.entries(obj).filter(([k]) => known.has(k))), ...(v !== null && typeof v === "object" && !Array.isArray(v) ? (v as Record<string, unknown>) : {}) })} />
        </span>
      ) : null}
    </span>
  );
}
