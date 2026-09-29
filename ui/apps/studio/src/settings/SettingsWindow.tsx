/**
 * The settings window: every setting a run is started from, in a window of its own, modelled on VS
 * Code's settings editor (docs/design/14-studio-shell.md §4).
 *
 * It replaces the 280 px settings sidebar, which put a hundred and forty fields in a column too
 * narrow to read one of them and cut its own Run button off at 1,280 px. Here the list has the
 * page's width, a search box that looks at every word the engine publishes about a setting, a tree
 * of the engine's own groups, and two filters: **Modified** (what this scenario and your edits set)
 * and **Unsupported** (what the engine reads nothing from, kept out of the way but never hidden
 * without saying so).
 *
 * The form is generated from the engine's published settings surface, and every row carries the
 * `x-status` the engine's `KEY_STATUS` table assigns it, so a field the engine does not act on is
 * marked with the engine's own sentence about it. Apply lists any edited field of that kind, so an
 * edit that changes nothing is never silently accepted.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ValidationError } from "@vwp/protocol";

import { Identifier } from "../components/Identifier.js";
import { useStudio } from "../state/store.js";
import {
  applyDraft,
  checkDraft,
  discardEdits,
  editSetting,
  inertEdits,
  loadPreset,
  presetsOf,
  revertStaged,
  runSettingsAction,
  runWithDraft,
  undoEdit,
} from "../state/settings.js";
import { changedPointers, getPointer } from "../lib/schema.js";
import {
  buildTree,
  errorPointer,
  errorsByField,
  isEdited,
  isModified,
  isUnsupported,
  matches,
  settingsFields,
  type Field,
} from "./model.js";
import { JsonView } from "./JsonView.js";
import { SettingRow } from "./SettingRow.js";

const SCENARIO_SECTION = "settings-scenario";

function ProblemList({ items, onJump }: { items: readonly ValidationError[]; onJump: (pointer: string) => void }): React.JSX.Element | null {
  if (items.length === 0) return null;
  return (
    <div className="note err" data-testid="validation-err">
      {items.map((e, i) => (
        <div key={`${e.path}-${i}`}>
          <button type="button" className="linklike" onClick={() => onJump(errorPointer(e.path))}>
            <code>{e.path}</code>
          </button>{" "}
          — {e.message}
          {e.hint ? <span className="faint"> ({e.hint})</span> : null}
        </div>
      ))}
    </div>
  );
}

export function SettingsWindow({
  detached = false,
  onClose,
  onDetach,
}: {
  /** True in the browser window of its own, which has no map and no stream. */
  detached?: boolean;
  onClose: () => void;
  /** Open this window in a browser window of its own. Absent where it already is one. */
  onDetach?: () => void;
}): React.JSX.Element {
  const scenario = useStudio((s) => s.scenario);
  const draft = useStudio((s) => s.draft);
  const scenarioHash = useStudio((s) => s.scenarioHash);
  const schema = useStudio((s) => s.scenarioSchema);
  const extras = useStudio((s) => s.scenarioExtras);
  const list = useStudio((s) => s.scenarioList);
  const validation = useStudio((s) => s.validation);
  const hello = useStudio((s) => s.hello);
  const outputDigest = useStudio((s) => s.run.outputDigest);
  const devDetails = useStudio((s) => s.devDetails);
  const busy = useStudio((s) => s.settingsBusy);
  const message = useStudio((s) => s.settingsMessage);

  const [query, setQuery] = useState("");
  const [modifiedOnly, setModifiedOnly] = useState(false);
  const [showUnsupported, setShowUnsupported] = useState(false);
  const [view, setView] = useState<"form" | "json">("form");
  const [active, setActive] = useState<string>(SCENARIO_SECTION);
  const search = useRef<HTMLInputElement | null>(null);
  const content = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    search.current?.focus();
  }, []);

  const { fields, source } = useMemo(
    () => settingsFields(extras.fields, schema, scenario),
    [extras.fields, schema, scenario],
  );
  const edits = useMemo(() => (draft === null ? [] : changedPointers(scenario, draft)), [scenario, draft]);
  const dirty = edits.length > 0;
  const unsupportedCount = useMemo(() => fields.filter(isUnsupported).length, [fields]);
  const modifiedCount = useMemo(
    () => fields.filter((f) => isEdited(f, edits) || isModified(f, getPointer(draft, f.pointer))).length,
    [fields, edits, draft],
  );
  const shown = useMemo(
    () =>
      fields.filter(
        (f) =>
          (showUnsupported || !isUnsupported(f)) &&
          matches(f, query) &&
          (!modifiedOnly || isEdited(f, edits) || isModified(f, getPointer(draft, f.pointer))),
      ),
    [fields, showUnsupported, query, modifiedOnly, edits, draft],
  );
  const tree = useMemo(() => buildTree(shown, extras.groups), [shown, extras.groups]);
  const filtering = query.trim() !== "" || modifiedOnly;

  // A new search starts at its first match, not wherever the list was scrolled to.
  useEffect(() => {
    content.current?.scrollTo({ top: 0 });
  }, [query, modifiedOnly, showUnsupported]);
  const problems = useMemo(() => errorsByField(fields, validation?.errors ?? []), [fields, validation]);
  const inert = useMemo(() => inertEdits(fields, edits), [fields, edits]);
  const presets = presetsOf(list);
  const durationS = (() => {
    const d = getPointer(draft, "/time/duration_s");
    return typeof d === "number" ? d : undefined;
  })();

  /** Scroll the list to a section or a setting, clearing a search that would hide it. */
  const jumpTo = useCallback((id: string) => {
    setView("form");
    requestAnimationFrame(() => {
      const el = document.getElementById(id);
      el?.scrollIntoView({ block: "start" });
      setActive(id);
    });
  }, []);

  const jumpToField = useCallback(
    (pointer: string) => {
      const target = fields
        .filter((f) => pointer === f.pointer || pointer.startsWith(`${f.pointer}/`))
        .sort((a, b) => b.pointer.length - a.pointer.length)[0];
      if (!target) return;
      setQuery("");
      setModifiedOnly(false);
      if (isUnsupported(target)) setShowUnsupported(true);
      const id = `setting${target.pointer.replace(/\W/g, "_")}`;
      jumpTo(id);
      requestAnimationFrame(() => {
        const input = document.getElementById(`f${target.pointer.replace(/\W/g, "_")}`) as HTMLElement | null;
        input?.focus();
      });
    },
    [fields, jumpTo],
  );

  // The tree follows the scroll: the section whose heading was last passed is the current one.
  const onScroll = useCallback(() => {
    const host = content.current;
    if (!host) return;
    const top = host.getBoundingClientRect().top;
    let current = SCENARIO_SECTION;
    for (const el of host.querySelectorAll<HTMLElement>("[data-section]")) {
      if (el.getBoundingClientRect().top - top <= 24) current = el.id;
      else break;
    }
    setActive((prev) => (prev === current ? prev : current));
  }, []);

  const staged = extras.staged;
  const sourceLine =
    source === "published" ? (
      <>
        {fields.length} settings, each with what this engine does with it.
        {devDetails ? <span className="faint"> From <code>scenario.get {"{with_schema:true}"}</code>.</span> : null}
      </>
    ) : source === "schema" ? (
      <>These {fields.length} fields are the ones this engine published.</>
    ) : (
      <>
        These are the standard scenario fields. This engine did not publish its own list, so it may accept settings
        that are not shown here, or ignore some that are. Press <b>Check</b> and it will say which.
      </>
    );

  return (
    <div className={detached ? "settings-window detached" : "settings-window"} data-testid="settings-window">
      <div className="settings-head">
        <h2>Settings</h2>
        <div className="settings-search">
          <input
            ref={search}
            type="search"
            placeholder={`Search ${fields.length} settings by name, description or path`}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Escape" && query !== "") {
                e.stopPropagation();
                setQuery("");
              }
            }}
            data-testid="settings-filter"
            aria-label="Search settings"
          />
          <span className="faint settings-count" data-testid="settings-count" aria-live="polite">
            {query !== "" || modifiedOnly ? `${shown.length} found` : ""}
          </span>
        </div>
        <div className="settings-filters" role="group" aria-label="Filters">
          <button
            type="button"
            className={modifiedOnly ? "toggle on" : "toggle"}
            aria-pressed={modifiedOnly}
            onClick={() => setModifiedOnly((v) => !v)}
            data-testid="settings-modified"
            title="Only settings this scenario sets away from the default, and your unapplied edits"
          >
            Modified <span className="count">{modifiedCount}</span>
          </button>
          <button
            type="button"
            className={showUnsupported ? "toggle on" : "toggle"}
            aria-pressed={showUnsupported}
            onClick={() => setShowUnsupported((v) => !v)}
            data-testid="settings-unsupported"
            title={
              unsupportedCount === 0
                ? "Every setting this engine publishes changes the run; there is nothing unsupported to show"
                : "Also show the settings this engine accepts but does not act on"
            }
          >
            Unsupported <span className="count">{unsupportedCount}</span>
          </button>
        </div>
        <div className="segmented" role="group" aria-label="View">
          <button type="button" className={view === "form" ? "on" : ""} aria-pressed={view === "form"} onClick={() => setView("form")} data-testid="settings-view-form">
            Form
          </button>
          <button type="button" className={view === "json" ? "on" : ""} aria-pressed={view === "json"} onClick={() => setView("json")} data-testid="settings-view-json">
            JSON
          </button>
        </div>
        {onDetach ? (
          <button type="button" className="icon-button" onClick={onDetach} data-testid="settings-detach" title="Open the settings in a browser window of their own" aria-label="Open in a new window">
            ⧉
          </button>
        ) : null}
        <button type="button" className="icon-button" onClick={onClose} data-testid="settings-close" title="Close the settings (Esc)" aria-label="Close the settings">
          ×
        </button>
      </div>

      <div className="settings-body">
        <nav className="settings-tree" aria-label="Setting groups" data-testid="settings-tree">
          <button
            type="button"
            className={active === SCENARIO_SECTION ? "tree-item on" : "tree-item"}
            onClick={() => {
              setQuery("");
              setModifiedOnly(false);
              jumpTo(SCENARIO_SECTION);
            }}
          >
            Scenario
          </button>
          {tree.map((g) => (
            <div key={g.id} className="tree-group">
              <button
                type="button"
                className={active === g.id || g.sections.some((s) => s.id === active) ? "tree-item group on" : "tree-item group"}
                onClick={() => jumpTo(g.id)}
                data-testid="settings-tree-group"
                data-group={g.name}
              >
                <span className="grow">{g.name}</span>
                <span className="count">{g.count}</span>
              </button>
              {g.sections.length > 1
                ? g.sections.map((s) => (
                    <button key={s.id} type="button" className={active === s.id ? "tree-item sub on" : "tree-item sub"} onClick={() => jumpTo(s.id)}>
                      <span className="grow">{s.label}</span>
                      <span className="count">{s.fields.length}</span>
                    </button>
                  ))
                : null}
            </div>
          ))}
        </nav>

        <div className="settings-content" ref={content} onScroll={onScroll} data-testid="scenario-panel">
          {view === "json" ? (
            <JsonView />
          ) : (
            <>
              {validation ? (
                <div className="settings-check">
                  <span className={`pill ${validation.valid ? "ok" : "err"}`} data-testid="validation-state">
                    {validation.valid ? "ready to run" : "needs fixing"}
                  </span>
                  {validation.valid && validation.warnings.length === 0 ? <span className="help"> The engine accepts these settings.</span> : null}
                  <ProblemList items={validation.errors} onJump={jumpToField} />
                  {validation.warnings.length > 0 ? (
                    <div className="note" data-testid="validation-warn">
                      {validation.warnings.map((w, i) => (
                        <div key={i}>
                          <code>{w.path}</code> — {w.message}
                        </div>
                      ))}
                    </div>
                  ) : null}
                </div>
              ) : null}
              {/* The scenario's identity and the ready-made list are not settings: a search or a
                  filter shows the settings it found, first. */}
              {!filtering ? (
              <section className="settings-scenario" id={SCENARIO_SECTION} data-section="scenario">
                <h3>Scenario</h3>
                <div className={source === "builtin" ? "note" : "note info"} data-testid="schema-source">
                  {sourceLine}
                </div>
                <dl className="kv">
                  <dt>running</dt>
                  <dd>{hello?.scenarioName ?? "—"}</dd>
                  <dt>identity</dt>
                  <dd>
                    <Identifier value={scenarioHash} label="scenario digest" testId="scenario-hash" />
                  </dd>
                  {outputDigest ? (
                    <>
                      <dt>result</dt>
                      <dd title="A digest of everything the finished run produced. Two runs with the same settings and seed give the same one.">
                        <Identifier value={outputDigest} label="output digest" testId="output-digest" />
                      </dd>
                    </>
                  ) : null}
                </dl>
                {presets.length > 0 ? (
                  <div className="presets" data-testid="presets">
                    <div className="dim">Start from a ready-made scenario:</div>
                    <div className="preset-grid">
                    {presets.map((item) => (
                      <div key={item.id} className="preset-row">
                        <span title={item.description}>
                          {item.name ?? item.id}
                          {presets.filter((o) => o.name === item.name).length > 1 ? <span className="faint"> ({item.id.split("/").pop()})</span> : null}
                          {(item as { running?: boolean }).running ? <span className="faint"> (running)</span> : null}
                        </span>
                        <button
                          type="button"
                          disabled={busy !== null}
                          data-testid="preset-load"
                          data-preset={item.name ?? item.id}
                          title="Load these settings into the form. The next run uses them."
                          onClick={() => void runSettingsAction("Load", () => loadPreset(item))}
                        >
                          Load
                        </button>
                      </div>
                    ))}
                    </div>
                  </div>
                ) : null}
              </section>
              ) : null}

              {tree.length === 0 ? (
                <p className="dim settings-empty" data-testid="settings-empty">
                  No setting matches{query !== "" ? ` “${query}”` : ""}
                  {modifiedOnly ? " among the modified ones" : ""}.
                  {!showUnsupported && unsupportedCount > 0 ? " Unsupported settings are hidden; turn on Unsupported to search them too." : ""}
                </p>
              ) : null}

              {tree.map((g) => (
                <section key={g.id} className="settings-group" aria-labelledby={`${g.id}-h`}>
                  <h3 id={g.id} data-section="group">
                    <span id={`${g.id}-h`}>{g.name}</span>
                  </h3>
                  {g.description ? <p className="help group-blurb">{g.description}</p> : null}
                  {g.sections.map((s) => (
                    <div key={s.id} className="settings-section">
                      {g.sections.length > 1 ? (
                        <h4 id={s.id} data-section="section">
                          {s.label}
                        </h4>
                      ) : null}
                      {s.fields.map((f: Field) => (
                        <SettingRow
                          key={f.pointer}
                          field={f}
                          sectionKey={s.key}
                          value={getPointer(draft, f.pointer)}
                          edited={isEdited(f, edits)}
                          modified={isModified(f, getPointer(draft, f.pointer))}
                          errors={problems.byPointer.get(f.pointer)}
                          devDetails={devDetails}
                          durationS={f.pointer === "/events" ? durationS : undefined}
                          canPick={!detached}
                          onEdit={editSetting}
                          onUndo={undoEdit}
                        />
                      ))}
                    </div>
                  ))}
                </section>
              ))}
            </>
          )}
        </div>
      </div>

      <div className="settings-foot">
        <div className="settings-foot-text">
          {staged ? (
            <div className="note warn-note" data-testid="staged-note">
              {staged.changed.length} setting{staged.changed.length === 1 ? "" : "s"} applied and waiting for the next run
              <span className="faint">: {staged.changed.slice(0, 4).join(", ")}{staged.changed.length > 4 ? ", …" : ""}</span>.{" "}
              <button
                type="button"
                className="linkbutton"
                disabled={busy !== null}
                data-testid="unstage"
                title="Forget the applied settings; the next run uses the scenario on screen now"
                onClick={() => void runSettingsAction("Revert", revertStaged)}
              >
                Revert to the running scenario
              </button>
            </div>
          ) : null}
          {dirty ? (
            <div className="dim" data-testid="edit-route">
              {edits.length} unapplied edit{edits.length === 1 ? "" : "s"}.{" "}
              {inert.length > 0 ? (
                <span className="err-text" data-testid="inert-edits">
                  {inert.length === 1 ? "One edit changes" : `${inert.length} edits change`} nothing in this build:{" "}
                  {inert.map((f) => f.label).join(", ")}.
                </span>
              ) : null}
            </div>
          ) : null}
          {message ? (
            <div className={`settings-message ${message.tone === "err" ? "err-text" : ""}`} data-testid="scenario-message" role="status">
              {message.text}
            </div>
          ) : null}
        </div>
        <button
          type="button"
          disabled={busy !== null}
          data-testid="validate"
          title="Ask the engine whether it accepts these settings, without changing anything"
          onClick={() => void runSettingsAction("Check", checkDraft)}
        >
          {busy === "Check" ? "Checking…" : "Check"}
        </button>
        <button
          type="button"
          disabled={busy !== null || !dirty}
          title="Put every setting back to what the engine holds"
          onClick={discardEdits}
          data-testid="discard-edits"
        >
          Discard
        </button>
        <button
          type="button"
          disabled={busy !== null || !dirty}
          data-testid="apply"
          title="Send the edited settings to the engine. The next run uses them."
          onClick={() => void runSettingsAction("Apply", applyDraft)}
        >
          {busy === "Apply" ? "Applying…" : "Apply"}
        </button>
        <button
          type="button"
          className="primary"
          disabled={busy !== null}
          data-testid="run-start"
          title={dirty ? "Apply your edits and start a run with them" : "Start the scenario from the beginning"}
          onClick={() =>
            void runSettingsAction("Run", async () => {
              const said = await runWithDraft();
              // You press Run to watch the run: the window gets out of the way when it has started.
              if (!detached) onClose();
              return said;
            })
          }
        >
          {busy === "Run" ? "Starting…" : dirty ? "Apply and run" : "Run"}
        </button>
      </div>
    </div>
  );
}
