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

import { memo, useEffect, useState } from "react";
import type { ValidationError } from "@vwp/protocol";

import { EventsEditor } from "../components/EventsEditor.js";
import { STATUS_BADGE, breadcrumbOf, describeValue, fieldPath, isUnsupported, oneLine, type Field } from "./model.js";
import { ChoiceSelect, ListEditor, MultiCheck, NumberWithPresets } from "./widgets.js";

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
    return <input {...common} type="checkbox" checked={value === true} onChange={(e) => onChange(e.target.checked)} />;
  }
  if (field.kind === "multi" && field.choices) {
    return <MultiCheck common={common} choices={field.choices} value={value} onChange={onChange} />;
  }
  if (field.kind === "list" && field.itemFields) {
    return (
      <ListEditor
        idBase={id}
        fields={field.itemFields}
        template={field.itemTemplate}
        value={value}
        onChange={onChange}
        noun={nounOf(field.label)}
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
      />
    );
  }
  if (field.kind === "json" || field.kind === "multi" || field.kind === "list") return <JsonWidget {...common} value={value} onChange={onChange} />;
  return (
    <input {...common} type="text" value={value === undefined || value === null ? "" : String(value)} onChange={(e) => onChange(e.target.value)} />
  );
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

function StatusBadge({ field }: { field: Field }): React.JSX.Element | null {
  if (!field.status) return null;
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
  const wide = f.kind === "list" || f.kind === "multi" || f.kind === "json" || f.pointer === "/events";
  return (
    <div
      className={`setting${unsupported ? " inactive" : ""}${edited ? " edited" : ""}${modified ? " modified" : ""}${invalid ? " invalid" : ""}${wide ? " wide" : ""}`}
      data-testid="setting"
      data-pointer={f.pointer}
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
            <Widget field={f} value={value} onChange={(v) => onEdit(f.pointer, v)} invalid={invalid} />
          )}
          {f.kind === "boolean" ? <span className="dim">{value === true ? "on" : "off"}</span> : null}
          {edited ? (
            <button type="button" className="icon-button tiny" data-testid="setting-undo" onClick={() => onUndo(f.pointer)} title="Undo this edit" aria-label="Undo this edit">
              ↶
            </button>
          ) : null}
        </div>
        <div className="setting-meta">
          {f.hasDefault ? (
            <span>
              Default <code>{describeValue(f.defaultValue)}</code>
              {modified ? (
                <>
                  {" "}
                  <button
                    type="button"
                    className="linkbutton"
                    data-testid="setting-reset"
                    onClick={() => onEdit(f.pointer, f.defaultValue)}
                    title={`Set this back to the engine's default (${describeValue(f.defaultValue)})`}
                  >
                    Reset
                  </button>
                </>
              ) : null}
            </span>
          ) : null}
          {f.recommended ? <span data-testid="setting-recommended" title="Recommended range">Rec. {f.recommended}</span> : f.range ? <span title="Allowed range">Range {f.range}</span> : null}
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
