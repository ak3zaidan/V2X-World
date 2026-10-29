/**
 * One setting in the settings window, as one compact row: the label and its unit on the left with a
 * one-line explanation under it, the control on the right with its default, recommended range and
 * a reset under it. The engine's full description and the dotted path are on hover (and behind
 * "More"), so the window reads as a list of controls rather than a page of paragraphs.
 *
 * Two markers, because there are two baselines and they answer different questions:
 *
 *  * **edited** — the value differs from what the engine holds for the next run. It is an unapplied
 *    change; the undo arrow puts back the engine's value.
 *  * **modified** — the value differs from the engine's default. It is what this scenario sets;
 *    *Reset* puts the default in (as an edit, applied like any other).
 */

import { memo, useContext, useEffect, useState } from "react";
import type { ValidationError } from "@vwp/protocol";

import { EventsEditor } from "../components/EventsEditor.js";
import { STATUS_BADGE, isComingSoon, breadcrumbOf, controlKind, describeValue, fieldPath, isUnsupported, oneLine, type Field } from "./model.js";
import { ScmsProviders } from "./ScmsProviders.js";
import { SettingsDocContext } from "./docContext.js";
import { getPointer, type Choice, type ItemField } from "../lib/schema.js";
import { PROVIDERS_POINTER } from "./scmsDeployment.js";
import { PARAMS_POINTER } from "./legacyParams.js";
import { ChoiceSelect, ListEditor, MapRows, MultiCheck, NumberWithPresets, RangePair } from "./widgets.js";

/** What one element of a list is called, from the list's title: "Attackers" → "Attacker". */
function nounOf(label: string): string {
  const word = label.trim().split(/\s+/)[0] || "Item";
  const singular = word.endsWith("ies") ? `${word.slice(0, -3)}y` : word.endsWith("s") ? word.slice(0, -1) : word;
  return singular.charAt(0).toUpperCase() + singular.slice(1);
}

function fieldId(field: Field): string {
  return `f${field.pointer.replace(/\W/g, "_")}`;
}

function Widget({
  field,
  value,
  onChange,
  invalid,
}: {
  field: Field;
  value: unknown;
  onChange: (v: unknown) => void;
  invalid: boolean;
}): React.JSX.Element {
  const id = fieldId(field);
  const common = { id, "aria-invalid": invalid || undefined, "aria-describedby": `${id}-desc` } as const;
  if (field.kind === "boolean") {
    // An unset key shows what the engine does with it, and nothing is written until it is clicked.
    const on = typeof value === "boolean" ? value : field.defaultValue === true;
    return <input {...common} type="checkbox" checked={on} onChange={(e) => onChange(e.target.checked)} />;
  }
  if (field.pointer === "/nodes/per_class" && field.valueFields && field.valueFields.length === 1) {
    return <ClassMap idBase={id} valueFields={field.valueFields} value={value} onChange={onChange} />;
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
  if (field.kind === "pair") {
    return <RangePair common={common} value={value} min={field.min} max={field.max} onChange={onChange} />;
  }
  if (field.kind === "multi" && field.choices) {
    return <MultiCheck common={common} choices={field.choices} value={value === undefined && field.hasDefault ? field.defaultValue : value} onChange={onChange} />;
  }
  if (field.pointer === PROVIDERS_POINTER && field.itemFields) {
    return <ScmsProviders idBase={id} fields={field.itemFields} onChange={onChange} />;
  }
  if (field.kind === "list" && field.itemFields) {
    return (
      <ListEditor
        idBase={id}
        fields={field.itemFields}
        template={field.itemTemplate}
        value={value}
        onChange={onChange}
        noun={field.noun ?? nounOf(field.label)}
      />
    );
  }
  if (field.kind === "enum" && field.choices) {
    return (
      <ChoiceSelect
        common={common}
        choices={field.choices}
        open={field.choicesOpen === true}
        nullable={field.nullable === true}
        value={value}
        onChange={onChange}
        {...(field.hasDefault ? { defaultValue: field.defaultValue } : {})}
        {...(field.emptyText ? { unsetLabel: field.emptyText } : {})}
      />
    );
  }
  if (field.kind === "enum") {
    const options = field.options ?? [];
    const current = value === undefined || value === null ? "" : String(value);
    return (
      <select {...common} value={current} onChange={(e) => onChange(e.target.value)}>
        {options.includes(current) ? null : <option value={current}>{current || "—"}</option>}
        {options.map((o) => (
          <option key={o} value={o}>
            {o}
          </option>
        ))}
      </select>
    );
  }
  // The master seed is an integer the document writes as a hexadecimal string, and a number input
  // cannot hold "0x…": a numeric kind with a string value is edited as text.
  if ((field.kind === "number" || field.kind === "integer") && typeof value !== "string") {
    return (
      <NumberWithPresets
        common={common}
        value={value}
        kind={field.kind}
        min={field.min}
        max={field.max}
        step={field.step}
        presets={field.presets}
        unit={field.unit}
        onChange={onChange}
        {...(field.hasDefault ? { defaultValue: field.defaultValue } : {})}
        {...(field.emptyText ? { emptyText: field.emptyText } : {})}
      />
    );
  }
  if (field.pointer === PARAMS_POINTER && value !== null && typeof value === "object") {
    // The older block's keys all have fields of their own; its JSON is there for the rare key that has not.
    return (
      <details className="json-collapsed">
        <summary className="dim">{Object.keys(value).length} values in JSON</summary>
        <JsonWidget {...common} value={value} onChange={onChange} />
      </details>
    );
  }
  if (field.kind === "json" || field.kind === "multi" || field.kind === "list" || field.kind === "map") return <JsonWidget {...common} value={value} onChange={onChange} />;
  return (
    <input {...common} type="text" value={value === undefined || value === null ? "" : String(value)} onChange={(e) => onChange(e.target.value)} />
  );
}

/** The vehicle classes the document defines, for a map keyed by class (`nodes.per_class`). */
function ClassMap({
  idBase,
  valueFields,
  value,
  onChange,
}: {
  idBase: string;
  valueFields: readonly ItemField[];
  value: unknown;
  onChange: (v: unknown) => void;
}): React.JSX.Element {
  const doc = useContext(SettingsDocContext);
  const classes = getPointer(doc, "/actors/vehicles/classes");
  const names = classes !== null && typeof classes === "object" && !Array.isArray(classes) ? Object.keys(classes as Record<string, unknown>) : [];
  const keys = names.length > 0 ? names : ["car", "truck", "bus", "motorcycle"];
  const keyChoices: Choice[] = keys.map((k) => ({ value: k, label: k, available: true }));
  return <MapRows idBase={idBase} keyChoices={keyChoices} keyOpen={false} valueFields={valueFields} value={value} onChange={onChange} />;
}

/**
 * A JSON value, edited as text. The text is kept while it does not parse, so a half-typed list is
 * not thrown away; the document gets the value only once it is valid JSON.
 */
function JsonWidget({
  value,
  onChange,
  ...rest
}: {
  id: string;
  value: unknown;
  onChange: (v: unknown) => void;
  "aria-invalid"?: boolean;
  "aria-describedby": string;
}): React.JSX.Element {
  const serialised = value === undefined ? "" : JSON.stringify(value);
  const [text, setText] = useState(serialised);
  const [bad, setBad] = useState(false);
  useEffect(() => {
    setText(serialised);
    setBad(false);
  }, [serialised]);
  return (
    <>
      <textarea
        {...rest}
        rows={Math.min(6, Math.max(1, Math.ceil(serialised.length / 70)))}
        value={text}
        className={bad ? "bad mono" : "mono"}
        spellCheck={false}
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
      {bad ? <div className="help err-text">Not valid JSON yet; the setting keeps its last valid value.</div> : null}
    </>
  );
}

/**
 * The engine's descriptions are written in Markdown — `code` and **bold** — and printed raw they
 * read as stray punctuation. Those two are rendered; nothing else is, and nothing is parsed as HTML.
 */
function Prose({ text }: { text: string }): React.JSX.Element {
  const parts = text.split(/(`[^`]+`|\*\*[^*]+\*\*)/g);
  return (
    <>
      {parts.map((p, i) =>
        p.startsWith("`") && p.endsWith("`") && p.length > 2 ? (
          <code key={i}>{p.slice(1, -1)}</code>
        ) : p.startsWith("**") && p.endsWith("**") && p.length > 4 ? (
          <b key={i}>{p.slice(2, -2)}</b>
        ) : (
          p
        ),
      )}
    </>
  );
}

/** A recommendation's first clause: "2 s (CAR 2 CAR …; the US …)" → "2 s". The rest is on hover. */
export function shortRec(text: string): string {
  const t = text.trim();
  const cut = t.search(/\s\(|;|\.\s/);
  const head = cut > 0 ? t.slice(0, cut) : t;
  return head.length > 48 ? `${head.slice(0, 46)}…` : head;
}

function StatusBadge({ field }: { field: Field }): React.JSX.Element | null {
  if (!field.status) return null;
  // "Partly applied" is said in the field's "more"; as a pill on every deployment row it was noise.
  if (field.status === "partial") return null;
  if (isComingSoon(field)) {
    return (
      <span className="field-status off" title={field.statusNote} data-testid="field-status" data-status="coming-soon">
        coming soon
      </span>
    );
  }
  const badge = STATUS_BADGE[field.status];
  if (!badge) return null;
  return (
    <span className={`field-status ${badge.cls}`} title={field.statusNote} data-testid="field-status" data-status={field.status}>
      {badge.label}
    </span>
  );
}

export interface SettingRowProps {
  readonly field: Field;
  readonly sectionKey: string;
  readonly value: unknown;
  readonly edited: boolean;
  readonly modified: boolean;
  readonly errors: readonly ValidationError[] | undefined;
  readonly devDetails: boolean;
  readonly durationS: number | undefined;
  readonly canPick: boolean;
  /** Why the field does not apply to the document as it stands (`x-relevant-when`). */
  readonly irrelevant?: string;
  /** The value the older parameter block gives this field while it is unset here. */
  readonly inherited?: unknown;
  readonly onEdit: (pointer: string, value: unknown) => void;
  readonly onUndo: (pointer: string) => void;
}

function SettingRowImpl({
  field: f,
  sectionKey,
  value,
  edited,
  modified,
  errors,
  devDetails,
  durationS,
  canPick,
  irrelevant,
  inherited,
  onEdit,
  onUndo,
}: SettingRowProps): React.JSX.Element {
  const id = fieldId(f);
  const crumb = breadcrumbOf(f, sectionKey);
  const unsupported = isUnsupported(f);
  const invalid = errors !== undefined && errors.length > 0;
  const [more, setMore] = useState(false);
  const line = oneLine(f.explain ?? f.help);
  const full = [f.explain, f.help].filter((t): t is string => !!t && t.trim() !== "").join("\n\n");
  const hasMore = full.replace(/[`*]/g, "").replace(/\s+/g, " ").trim() !== line || (f.status !== "wired" && !!f.statusNote);
  const defaultWords = f.hasDefault ? (f.choices?.find((c) => String(c.value) === String(f.defaultValue))?.label ?? describeValue(f.defaultValue)) : "";
  const recRaw = f.recommended ? shortRec(f.recommended) : undefined;
  // A recommendation that is a menu value ("frost-p256") reads as the menu's words.
  const rec = recRaw === undefined ? undefined : (f.choices?.find((c) => String(c.value) === recRaw.replace(/\.$/, ""))?.label ?? recRaw);
  const flat = (t: string): string => t.toLowerCase().replace(/[,\s]/g, "");
  const defaultSpellings = f.hasDefault
    ? [defaultWords, describeValue(f.defaultValue), f.defaultValue === true ? "on" : f.defaultValue === false ? "off" : ""].filter((t) => t !== "").map(flat)
    : [];
  const recFlat = rec === undefined ? "" : flat(rec).replace(/\.$/, "");
  const recSame =
    rec !== undefined && (/^unset$/i.test(recFlat) || defaultSpellings.some((d) => recFlat.startsWith(d) || d.startsWith(recFlat)));
  if (f.pointer === PROVIDERS_POINTER && f.itemFields) {
    // The provider cards carry their own words; the sub-heading above them names them.
    return (
      <div className={`setting wide bare${edited ? " edited" : ""}${invalid ? " invalid" : ""}`} data-testid="setting" data-pointer={f.pointer} data-kind="providers" data-edited={edited ? "true" : "false"} id={`setting${f.pointer.replace(/\W/g, "_")}`}>
        <div className="setting-marker" aria-hidden="true" />
        <div className="setting-right">
          <ScmsProviders idBase={id} fields={f.itemFields} onChange={(v) => onEdit(f.pointer, v)} />
          {invalid ? (
            <div className="setting-error" role="alert" data-testid="setting-error">
              {errors.map((e, i) => (
                <div key={i}>{e.message}</div>
              ))}
            </div>
          ) : null}
        </div>
      </div>
    );
  }
  const wide = f.kind === "list" || f.kind === "multi" || f.kind === "json" || f.kind === "map" || f.pointer === "/events";
  return (
    <div
      className={`setting${unsupported || irrelevant ? " inactive" : ""}${edited ? " edited" : ""}${modified ? " modified" : ""}${invalid ? " invalid" : ""}${wide ? " wide" : ""}`}
      data-testid="setting"
      data-pointer={f.pointer}
      data-kind={f.pointer === "/events" ? "events" : controlKind(f)}
      data-relevant={irrelevant ? "false" : "true"}
      data-edited={edited ? "true" : "false"}
      data-modified={modified ? "true" : "false"}
      id={`setting${f.pointer.replace(/\W/g, "_")}`}
    >
      <div className="setting-marker" aria-hidden="true" />
      <div className="setting-label">
        <label className="setting-title" htmlFor={id} title={`${fieldPath(f)}${full ? `\n\n${full.replace(/[`*]/g, "")}` : ""}`}>
          {crumb ? <span className="setting-crumb">{crumb} › </span> : null}
          <b>{f.label}</b>
          {f.unit ? <span className="unit"> {f.unit}</span> : null}
          {edited ? (
            <span className="setting-dot edited" title="Changed here and not applied yet" aria-label="edited" />
          ) : modified ? (
            <span className="setting-dot" title="This scenario sets a value other than the engine's default" aria-label="modified" />
          ) : null}
          <StatusBadge field={f} />
        </label>
        <div className="setting-desc" id={`${id}-desc`}>
          {irrelevant ? (
            <span className="setting-irrelevant" data-testid="setting-irrelevant">
              {irrelevant}{" "}
            </span>
          ) : null}
          <span data-testid="setting-explain">{line}</span>
          {hasMore ? (
            <button type="button" className="linkbutton more" onClick={() => setMore((m) => !m)} aria-expanded={more}>
              {more ? "less" : "more"}
            </button>
          ) : null}
        </div>
        {more ? (
          <div className="setting-more">
            {full ? <Prose text={full} /> : null}
            {f.statusNote && f.status !== "wired" ? (
              <div className="status-note">
                <Prose text={f.statusNote} />
              </div>
            ) : null}
            <div className="setting-path">
              <code>{fieldPath(f)}</code>
              {devDetails ? <code className="faint"> {f.pointer}</code> : null}
            </div>
          </div>
        ) : null}
        {f.status === "wired" && edited && f.statusNote && f.statusNote.trim() !== (f.help ?? "").trim() ? (
          <div className="setting-more faint" data-testid="wired-note">
            <Prose text={oneLine(f.statusNote)} />
          </div>
        ) : null}
      </div>
      <div className="setting-right">
        <div className={f.kind === "boolean" ? "setting-control inline" : "setting-control"}>
          {f.pointer === "/events" ? (
            <EventsEditor value={value} onChange={(v) => onEdit(f.pointer, v)} durationS={durationS} canPick={canPick} />
          ) : (
            <Widget
              field={inherited !== undefined ? { ...f, hasDefault: true, defaultValue: inherited } : f}
              value={value}
              onChange={(v) => onEdit(f.pointer, v)}
              invalid={invalid}
            />
          )}
          {f.kind === "boolean" ? <span className="dim">{(typeof value === "boolean" ? value : f.defaultValue === true) ? "on" : "off"}</span> : null}

          {edited ? (
            <button type="button" className="icon-button tiny" data-testid="setting-undo" onClick={() => onUndo(f.pointer)} title="Undo this edit" aria-label="Undo this edit">
              ↶
            </button>
          ) : null}
        </div>
        <div className="setting-meta">
          {inherited !== undefined && value === undefined ? (
            <span className="inherited" data-testid="setting-inherited" title="Set in the older parameter block (security.protocol.params); a value here wins">
              from the older block
            </span>
          ) : null}
          {f.hasDefault && modified ? (
            <span>
              Default {f.choices ? defaultWords : <code>{defaultWords}</code>}{" "}
              <button
                type="button"
                className="linkbutton"
                data-testid="setting-reset"
                onClick={() => onEdit(f.pointer, f.defaultValue)}
                title={`Set this back to the engine's default (${defaultWords})`}
              >
                Reset
              </button>
            </span>
          ) : null}
          {f.emptyText && !f.choices && value !== undefined && value !== null ? (
            <button type="button" className="linkbutton" data-testid="setting-clear" onClick={() => onEdit(f.pointer, undefined)} title="Remove this value">
              Clear ({f.emptyText})
            </button>
          ) : null}
          {rec && !recSame ? (
            <span data-testid="setting-recommended" title={`Recommended: ${f.recommended}`}>
              Rec. {rec}
            </span>
          ) : null}
        </div>
        {invalid ? (
          <div className="setting-error" role="alert" data-testid="setting-error">
            {errors.map((e, i) => (
              <div key={i}>
                {e.message}
                {e.hint ? <span className="faint"> ({e.hint})</span> : null}
              </div>
            ))}
          </div>
        ) : null}
      </div>
    </div>
  );
}

/** Memoised: a keystroke in one row re-renders that row, not the other hundred and forty. */
export const SettingRow = memo(SettingRowImpl);
