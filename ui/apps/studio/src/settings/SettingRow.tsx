/**
 * One setting in the settings window, laid out the way VS Code lays one out: the title (with the
 * section path in front of it and the unit after it), the engine's status, the description, the
 * control, and a line with the default, the dotted path and the per-setting reset.
 *
 * Two markers, because there are two baselines and they answer different questions:
 *
 *  * **edited** — the value differs from what the engine holds for the next run. It is an unapplied
 *    change; *Undo edit* puts back the engine's value.
 *  * **modified** — the value differs from the engine's default. It is what this scenario sets;
 *    *Reset to default* puts the default in (as an edit, applied like any other).
 */

import { memo, useEffect, useState } from "react";
import type { ValidationError } from "@vwp/protocol";

import { EventsEditor } from "../components/EventsEditor.js";
import { STATUS_BADGE, breadcrumbOf, describeValue, fieldPath, isUnsupported, type Field } from "./model.js";

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
      <input
        {...common}
        type="number"
        value={typeof value === "number" ? value : ""}
        min={field.min}
        max={field.max}
        step={field.step ?? (field.kind === "integer" ? 1 : "any")}
        onChange={(e) => onChange(e.target.value === "" ? undefined : Number(e.target.value))}
      />
    );
  }
  if (field.kind === "json") return <JsonWidget {...common} value={value} onChange={onChange} />;
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
        rows={Math.min(8, Math.max(1, Math.ceil(serialised.length / 90)))}
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
      {bad ? <div className="help err-text">Not valid JSON yet — the setting keeps its last valid value.</div> : null}
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
  return (
    <div
      className={`setting${unsupported ? " inactive" : ""}${edited ? " edited" : ""}${modified ? " modified" : ""}${invalid ? " invalid" : ""}`}
      data-testid="setting"
      data-pointer={f.pointer}
      data-edited={edited ? "true" : "false"}
      data-modified={modified ? "true" : "false"}
      id={`setting${f.pointer.replace(/\W/g, "_")}`}
    >
      <div className="setting-marker" aria-hidden="true" />
      <div className="setting-main">
        <label className="setting-title" htmlFor={id}>
          {crumb ? <span className="setting-crumb">{crumb}: </span> : null}
          <b>{f.label}</b>
          {f.unit ? <span className="unit"> [{f.unit}]</span> : null}
          {edited ? (
            <span className="setting-flag edited-flag" title="Changed here and not applied yet">
              edited
            </span>
          ) : modified ? (
            <span className="setting-flag" title="This scenario sets a value other than the engine's default">
              modified
            </span>
          ) : null}
          <StatusBadge field={f} />
        </label>
        <div className="setting-desc" id={`${id}-desc`}>
          <Prose text={f.help ?? ""} />
          {f.status && f.status !== "wired" && f.statusNote ? (
            <div className="status-note">
              <Prose text={f.statusNote} />
            </div>
          ) : null}
          {/* A fully applied field's note says what the engine does with it, including where a value
              changes nothing (radio.tiers.phy "abstract" under a medium MAC, security.envelope on a
              BSM run). It is shown once the field is edited — the moment the user needs it — rather
              than on all hundred and forty rows. */}
          {f.status === "wired" && edited && f.statusNote && f.statusNote.trim() !== (f.help ?? "").trim() ? (
            <div className="faint" data-testid="wired-note">
              <Prose text={f.statusNote} />
            </div>
          ) : null}
        </div>
        <div className={f.kind === "boolean" ? "setting-control inline" : "setting-control"}>
          {f.pointer === "/events" ? (
            <EventsEditor value={value} onChange={(v) => onEdit(f.pointer, v)} durationS={durationS} canPick={canPick} />
          ) : (
            <Widget field={f} value={value} onChange={(v) => onEdit(f.pointer, v)} invalid={invalid} />
          )}
          {f.kind === "boolean" ? <span className="dim">{value === true ? "on" : "off"}</span> : null}
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
        <div className="setting-meta">
          {f.hasDefault ? <span>Default: <code>{describeValue(f.defaultValue)}</code></span> : null}
          {f.range ? <span>Range: {f.range}</span> : null}
          <span className="setting-path" title="How a scenario file spells this setting">
            <code>{fieldPath(f)}</code>
          </span>
          {devDetails ? <span className="faint"><code>{f.pointer}</code></span> : null}
          <span className="grow" />
          {edited ? (
            <button type="button" className="linkbutton" data-testid="setting-undo" onClick={() => onUndo(f.pointer)} title="Put back the value the engine holds for the next run">
              Undo edit
            </button>
          ) : null}
          {modified && f.hasDefault ? (
            <button
              type="button"
              className="linkbutton"
              data-testid="setting-reset"
              onClick={() => onEdit(f.pointer, f.defaultValue)}
              title={`Set this back to the engine's default (${describeValue(f.defaultValue)})`}
            >
              Reset to default
            </button>
          ) : null}
        </div>
      </div>
    </div>
  );
}

/** Memoised: a keystroke in one row re-renders that row, not the other hundred and forty. */
export const SettingRow = memo(SettingRowImpl);
