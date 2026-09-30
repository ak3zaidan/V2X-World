/**
 * The whole scenario as JSON, round-tripping with the form.
 *
 * The text is the draft pretty-printed. Typing in it replaces the draft the moment the text parses
 * to an object, so the form (and Apply) see the edit at once; while it does not parse, the text is
 * kept, the draft keeps its last good value, and the parser's complaint is shown under the box. An
 * edit made in the form while this view is closed is in the text when it opens, because the text is
 * derived from the same draft.
 *
 * JSON rather than YAML: the page has no YAML parser, and writing one for this would be a second
 * scenario loader that could disagree with the engine's. The engine accepts the JSON form of the
 * same document, and a scenario file written as JSON loads like one written as YAML.
 */

import { useEffect, useRef, useState } from "react";

import { useStudio } from "../state/store.js";

export function JsonView(): React.JSX.Element {
  const draft = useStudio((s) => s.draft);
  const setDraft = useStudio((s) => s.setDraft);
  const serialised = draft === null ? "" : JSON.stringify(draft, null, 2);
  const [text, setText] = useState(serialised);
  const [error, setError] = useState<string | null>(null);
  // The last text this view produced a draft from, so a draft change that came *from* the text does
  // not rewrite the text under the cursor (re-indenting it and moving the caret).
  const fromText = useRef<string | null>(null);

  useEffect(() => {
    if (fromText.current !== null && fromText.current === serialised) return;
    fromText.current = null;
    setText(serialised);
    setError(null);
  }, [serialised]);

  return (
    <div className="json-view">
      <p className="help">
        The whole scenario as the engine will receive it. Edit it here or in the form; each shows the other&apos;s
        changes. It is applied with <b>Apply</b> or <b>Run</b>, like an edit in the form.
      </p>
      <textarea
        className={error ? "json-text bad" : "json-text"}
        data-testid="settings-json"
        aria-label="The scenario as JSON"
        spellCheck={false}
        value={text}
        onChange={(e) => {
          const next = e.target.value;
          setText(next);
          try {
            const parsed: unknown = JSON.parse(next);
            if (parsed === null || typeof parsed !== "object" || Array.isArray(parsed)) {
              setError("A scenario is a JSON object: { … }.");
              return;
            }
            setError(null);
            fromText.current = JSON.stringify(parsed, null, 2);
            setDraft(parsed as Record<string, unknown>);
          } catch (err) {
            setError(err instanceof Error ? err.message : String(err));
          }
        }}
      />
      {error ? (
        <div className="note err" role="alert" data-testid="settings-json-error">
          Not valid JSON yet — the settings keep their last valid value. {error}
        </div>
      ) : null}
    </div>
  );
}
