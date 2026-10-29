/**
 * The settings window's controls for a value from a known set, and for a number with named values.
 *
 * The owner's review (2026-10-06): "a lot of fields the user needs to manually fill out … the user
 * doesn't really know what is supposed to go there". Every set of values the engine knows is
 * published with words (`x-options`: a label and one sentence per value) and every number that has
 * well-known values with its presets (`x-presets`); these controls show them. Nothing here names a
 * model id: the lists come from the engine, so a model added to the engine appears here unedited.
 */

import { useEffect, useState } from "react";

import type { Choice, ItemField, NumberPreset } from "../lib/schema.js";
import { getPointer, setPointer } from "../lib/schema.js";

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
}: {
  common: Common;
  choices: readonly Choice[];
  open: boolean;
  nullable: boolean;
  value: unknown;
  onChange: (v: unknown) => void;
  compact?: boolean;
}): React.JSX.Element {
  const known = choices.find((c) => sameValue(value, c.value));
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
  return (
    <span className={compact ? "choice compact" : "choice"}>
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
        {unset || nullable ? (
          <option value={UNSET}>{nullable ? "Engine default" : "— choose —"}</option>
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
      <OptionExplainer choices={choices} />
      {known?.us === false ? (
        <span className="badge-not-us" data-testid="not-us" title="The deployed US system does not use this value">
          not US
        </span>
      ) : null}
      {!compact && known?.description ? (
        <span className="choice-desc" data-testid="choice-desc">
          {known.description}
        </span>
      ) : null}
    </span>
  );
}

/**
 * An "i" button beside a menu that lists every option with its sentence, its US badge and, for a
 * disabled option, the reason (08-settings-ux-and-gaps §4.2 item 4). A disabled `<option title>`
 * tooltip is not shown by Chrome or Safari on macOS, so the reason needs a place of its own, and a
 * reader should see what each value does before choosing one.
 */
export function OptionExplainer({ choices }: { choices: readonly Choice[] }): React.JSX.Element | null {
  const [open, setOpen] = useState(false);
  if (choices.length === 0) return null;
  return (
    <span className="option-explainer">
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
 * A map edited one row per key (08-settings-ux-and-gaps §4.2 item 1): the key is a menu of the keys
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
      {keys.length === 0 ? <div className="dim">None.</div> : null}
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
}): React.JSX.Element {
  const input = (
    <input
      {...common}
      type="number"
      value={typeof value === "number" ? value : ""}
      min={min}
      max={max}
      step={step ?? (kind === "integer" ? 1 : "any")}
      onChange={(e) => onChange(e.target.value === "" ? undefined : Number(e.target.value))}
    />
  );
  if (!presets || presets.length === 0) return input;
  const active = presets.find((p) => p.value === value);
  return (
    <span className="number-presets">
      <span className="preset-chips" role="group" aria-label="Common values">
        {presets.map((p) => (
          <button
            key={p.label}
            type="button"
            className={p === active ? "preset-chip active" : "preset-chip"}
            aria-pressed={p === active}
            title={`${p.value}${unit ? ` ${unit}` : ""}${p.note ? ` — ${p.note}` : ""}`}
            data-testid="preset-chip"
            onClick={() => onChange(p.value)}
          >
            {p.label}
          </button>
        ))}
      </span>
      <span className="preset-custom">
        {input}
        <span className="faint">{active ? active.note : "custom value"}</span>
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
            <input type="checkbox" checked={list.includes(v)} disabled={!c.available && !list.includes(v)} onChange={(e) => toggle(v, e.target.checked)} />
            <span>
              <b>{c.label}</b>
              {c.description ? <span className="faint"> — {c.description}</span> : null}
            </span>
          </label>
        );
      })}
      {extra.map((v) => (
        <label key={v} className="multi-item">
          <input type="checkbox" checked onChange={(e) => toggle(v, e.target.checked)} />
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
}: {
  id: string;
  field: ItemField;
  value: unknown;
  onChange: (v: unknown) => void;
}): React.JSX.Element {
  const common = { id, "aria-describedby": `${id}-desc` } as const;
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
}: {
  idBase: string;
  fields: readonly ItemField[];
  template: unknown;
  value: unknown;
  onChange: (v: unknown) => void;
  noun: string;
}): React.JSX.Element {
  const list = Array.isArray(value) ? (value as unknown[]) : [];
  // The choice first (it is what the element *is*), then the rest in the engine's order.
  const ordered = [...fields.filter((f) => f.choices), ...fields.filter((f) => !f.choices)];
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
      {list.length === 0 ? <div className="dim">None.</div> : null}
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
              return (
                <label key={f.key} className={f.choices || f.kind === "map" ? "list-cell wide" : "list-cell"} htmlFor={id} title={f.help}>
                  <span className="list-cell-label">
                    {f.label}
                    {f.unit ? <span className="unit"> [{f.unit}]</span> : null}
                  </span>
                  <ItemControl id={id} field={f} value={getPointer(item, f.key)} onChange={(v) => setItem(i, f.key, v)} />
                </label>
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
