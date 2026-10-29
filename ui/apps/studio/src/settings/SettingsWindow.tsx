/**
 * The settings window: every setting a run is started from, in a window of its own.
 *
 * A left navigation of sections in the order a researcher works (Run, Traffic, Radio & Channel,
 * Medium Access, Congestion Control, Messages, Networking, Security & Crypto, Certificates & SCMS, Backend &
 * Cellular, Attackers, Output; `model.ts` `RESEARCH_SECTIONS`), one section on the page at a time,
 * its Basic settings first and its Advanced ones behind one toggle. The search box looks across
 * every section. "Changes" lists what differs from the scenario the engine holds, each with an undo.
 *
 * Every row is generated from the engine's published settings surface (`scenario.get`): its label,
 * unit, explanation, default, recommended range, level and status. Nothing here names a setting
 * the engine did not publish, so an option the engine adds appears without an edit here; one no
 * section rule claims is shown under the engine's own group name after the research sections.
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
  RESTART_NOTE,
} from "../state/settings.js";
import { RestartNow } from "../components/RestartNow.js";
import { useRevealSetting } from "../lib/links.js";
import { changedPointers, getPointer } from "../lib/schema.js";
import {
  buildResearchTree,
  irrelevantReason,
  describeValue,
  errorPointer,
  errorsByField,
  refineError,
  fieldPath,
  isEdited,
  isModified,
  isBasic,
  isComingSoon,
  comingSoonFeatures,
  isUnsupported,
  matches,
  oneLine,
  researchSectionOf,
  settingsFields,
  type Field,
  type ResearchGroup,
} from "./model.js";
import { JsonView } from "./JsonView.js";
import { applyPatch } from "./quick.js";
import "../styles/quick.css";
import "../styles/settings.css";
import { SettingsDocContext } from "./docContext.js";
import { SettingRow } from "./SettingRow.js";
import { localUsVerdict } from "./scmsDeployment.js";
import { PARAMS_POINTER, inheritedValue, movableKeys, moveIntoFields } from "./legacyParams.js";

const SECTION_KEY = "v2xw.settings.section";
const ADVANCED_KEY = "v2xw.settings.advanced";

function load<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key);
    return raw === null ? fallback : (JSON.parse(raw) as T);
  } catch {
    return fallback;
  }
}
function save(key: string, value: unknown): void {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    /* private window: the choice lasts this session */
  }
}

function ProblemList({
  items,
  onJump,
  nameOf,
}: {
  items: readonly ValidationError[];
  onJump: (pointer: string) => void;
  nameOf: (pointer: string) => string | undefined;
}): React.JSX.Element | null {
  if (items.length === 0) return null;
  return (
    <div className="note err" data-testid="validation-err">
      {items.map(refineError).map((e, i) => (
        <div key={`${e.path}-${i}`}>
          <button type="button" className="linklike" onClick={() => onJump(errorPointer(e.path))} title={e.path}>
            {nameOf(errorPointer(e.path)) ?? <code>{e.path}</code>}
          </button>
          {": "}
          {e.message}
          {e.hint ? <span className="faint"> ({e.hint})</span> : null}
        </div>
      ))}
    </div>
  );
}

/** The section id a field is shown under (its research section, or its engine group's). */
function sectionIdOf(f: Field, tree: readonly ResearchGroup[]): string | undefined {
  return tree.find((g) => g.subs.some((s) => s.fields.includes(f)))?.id;
}

/**
 * Whether these settings are the deployed US SCMS (08-settings-ux-and-gaps §4.2 item 7), from the
 * engine's own verdict on the last Check (`scenario.validate` `us_system`).
 */
function UsBanner({
  verdict,
  local,
  protocol,
}: {
  verdict: { readonly isUs: boolean; readonly reasons: readonly string[] } | undefined;
  local: { readonly isUs: boolean; readonly reasons: readonly string[] };
  protocol: unknown;
}): React.JSX.Element {
  if (typeof protocol === "string" && !protocol.includes("scms")) {
    return (
      <div className="note us-banner" data-testid="us-banner" data-us="none">
        This run uses the European credential system; the US SCMS settings below do not apply.
      </div>
    );
  }
  // The form's own reading is live; the engine's (from the last Check) adds what the form cannot see.
  const v = local.isUs && verdict && !verdict.isUs ? verdict : local;
  return (
    <div className={`note us-banner${v.isUs ? " ok" : ""}`} data-testid="us-banner" data-us={v.isUs ? "true" : "false"}>
      {v.isUs ? (
        <>
          <b>US system.</b> <span className="dim">Everything here matches the deployed US SCMS.</span>
        </>
      ) : (
        <>
          <b>Not the US system:</b> {v.reasons.join(", ") || "a setting the US does not use"}.
        </>
      )}
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
  const studies = extras.studies;

  const [query, setQuery] = useState("");
  const [modifiedOnly, setModifiedOnly] = useState(false);
  const [showUnsupported, setShowUnsupported] = useState(false);
  const [view, setView] = useState<"form" | "json" | "changes">("form");
  const [active, setActiveState] = useState<string>(() => load(SECTION_KEY, "group-run"));
  const [advanced, setAdvanced] = useState<Record<string, boolean>>(() => load(ADVANCED_KEY, {}));
  const search = useRef<HTMLInputElement | null>(null);
  const content = useRef<HTMLDivElement | null>(null);

  const setActive = useCallback((id: string) => {
    setActiveState(id);
    save(SECTION_KEY, id);
    content.current?.scrollTo({ top: 0 });
  }, []);
  const toggleAdvanced = (id: string, on: boolean): void => {
    setAdvanced((prev) => {
      const next = { ...prev, [id]: on };
      save(ADVANCED_KEY, next);
      return next;
    });
  };

  useEffect(() => {
    search.current?.focus();
  }, []);

  const { fields, source } = useMemo(() => settingsFields(extras.fields, schema, scenario), [extras.fields, schema, scenario]);
  const edits = useMemo(() => (draft === null ? [] : changedPointers(scenario, draft)), [scenario, draft]);
  const dirty = edits.length > 0;
  const anyLevels = useMemo(() => fields.some((f) => f.level !== undefined), [fields]);
  const unsupportedCount = useMemo(() => fields.filter((f) => isUnsupported(f) && !isComingSoon(f)).length, [fields]);
  const soon = useMemo(() => comingSoonFeatures(fields), [fields]);
  const isMod = useCallback((f: Field) => isEdited(f, edits) || isModified(f, getPointer(draft, f.pointer)), [edits, draft]);
  const modifiedCount = useMemo(() => fields.filter(isMod).length, [fields, isMod]);

  // Every field the filters allow, in its section; the page then shows one section, or every
  // section's matches while a search or the Modified filter is on.
  const visible = useMemo(
    () =>
      fields.filter((f) =>
        isComingSoon(f)
          ? isEdited(f, edits) || getPointer(draft, f.pointer) !== undefined
          : showUnsupported || !isUnsupported(f) || isEdited(f, edits),
      ),
    [fields, showUnsupported, edits, draft],
  );
  const fullTree = useMemo(() => buildResearchTree(visible), [visible]);
  const filtering = query.trim() !== "" || modifiedOnly;
  const found = useMemo(
    () => (filtering ? visible.filter((f) => matches(f, query) && (!modifiedOnly || isMod(f))) : []),
    [filtering, visible, query, modifiedOnly, isMod],
  );
  const foundTree = useMemo(() => buildResearchTree(found), [found]);
  const current = fullTree.find((g) => g.id === active) ?? fullTree[0];
  const editedBySection = useMemo(() => {
    const out = new Map<string, number>();
    for (const f of fields) {
      if (!isEdited(f, edits)) continue;
      const id = sectionIdOf(f, fullTree);
      if (id) out.set(id, (out.get(id) ?? 0) + 1);
    }
    return out;
  }, [fields, edits, fullTree]);

  const movable = useMemo(() => movableKeys(draft, new Set(fields.map((f) => f.pointer))), [draft, fields]);
  const problems = useMemo(() => errorsByField(fields, validation?.errors ?? []), [fields, validation]);
  const inert = useMemo(() => inertEdits(fields, edits), [fields, edits]);
  const presets = presetsOf(list);
  const durationS = (() => {
    const d = getPointer(draft, "/time/duration_s");
    return typeof d === "number" ? d : undefined;
  })();

  const jumpToField = useCallback(
    (pointer: string) => {
      const target = fields
        .filter((f) => pointer === f.pointer || pointer.startsWith(`${f.pointer}/`))
        .sort((a, b) => b.pointer.length - a.pointer.length)[0];
      if (!target) return;
      setQuery("");
      setModifiedOnly(false);
      setView("form");
      if (isUnsupported(target)) setShowUnsupported(true);
      const tree = buildResearchTree(fields);
      const sec = sectionIdOf(target, tree);
      if (sec) {
        setActive(sec);
        if (!isBasic(target, anyLevels)) toggleAdvanced(sec, true);
      }
      requestAnimationFrame(() =>
        requestAnimationFrame(() => {
          document.getElementById(`setting${target.pointer.replace(/\W/g, "_")}`)?.scrollIntoView({ block: "center" });
          (document.getElementById(`f${target.pointer.replace(/\W/g, "_")}`) as HTMLElement | null)?.focus();
        }),
      );
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [fields, anyLevels, setActive],
  );
  // A link from elsewhere (the agent's list of the settings it changed) opens the window here.
  useRevealSetting(jumpToField, fields.length > 0);

  const row = (f: Field, sectionKey: string): React.JSX.Element => (
    <SettingsDocContext.Provider key={f.pointer} value={draft}>
    <SettingRow
      field={f}
      sectionKey={sectionKey}
      value={getPointer(draft, f.pointer)}
      edited={isEdited(f, edits)}
      modified={isModified(f, getPointer(draft, f.pointer))}
      errors={problems.byPointer.get(f.pointer)}
      devDetails={devDetails}
      durationS={f.pointer === "/events" ? durationS : undefined}
      canPick={!detached}
      irrelevant={irrelevantReason(f, draft, fields)}
      inherited={inheritedValue(draft, f.pointer)}
      onEdit={editSetting}
      onUndo={undoEdit}
    />
    {f.pointer === PARAMS_POINTER && movable.length > 0 ? (
      <div className="setting-move" data-testid="params-move">
        <span className="dim">
          {movable.length} of these {movable.length === 1 ? "value has a field" : "values have fields"} in the form.
        </span>
        <button type="button" className="btn small" onClick={() => moveIntoFields(draft, movable).forEach((w) => editSetting(w.pointer, w.value))}>
          Move into the form
        </button>
      </div>
    ) : null}
    </SettingsDocContext.Provider>
  );

  /**
   * A setting that does not apply while the document leaves it unset is left out (the pseudonym
   * distance under a time-based change); one the document sets stays, greyed, so no value is
   * hidden. A search shows everything.
   */
  const hiddenNow = (f: Field): boolean =>
    !filtering && getPointer(draft, f.pointer) === undefined && !isEdited(f, edits) && irrelevantReason(f, draft, fields) !== undefined;

  /** One section's sub-headings with the fields `keep` allows. */
  const subsOf = (g: ResearchGroup, keep: (f: Field) => boolean): React.JSX.Element[] =>
    g.subs
      .map((s) => ({ s, shown: s.fields.filter((f) => keep(f) && !hiddenNow(f)) }))
      .filter(({ shown }) => shown.length > 0)
      .map(({ s, shown }) => (
        <div key={s.key} className="settings-sub">
          {g.subs.length > 1 ? <h4 data-testid="settings-subheading">{s.label}</h4> : null}
          {g.subs.length > 1 && s.blurb ? <p className="faint settings-sub-blurb">{oneLine(s.blurb)}</p> : null}
          {shown.map((f) => row(f, s.key))}
        </div>
      ));

  const sectionPage = (g: ResearchGroup): React.JSX.Element => {
    const basic = (f: Field): boolean => isBasic(f, anyLevels) || isEdited(f, edits);
    const basicCount = g.subs.reduce((n, s) => n + s.fields.filter(basic).length, 0);
    const advCount = g.count - basicCount;
    const showAdv = advanced[g.id] === true || basicCount === 0;
    return (
      <section className="settings-group" aria-labelledby={`${g.id}-h`} data-testid="settings-section" data-section-id={g.id}>
        <div className="section-head">
          <h3 id={`${g.id}-h`}>{g.name}</h3>
          {g.blurb ? <span className="faint">{g.blurb}</span> : null}
        </div>
        {g.id === "group-run" ? runExtras : null}
        {g.id === "group-scms" ? (
          <UsBanner verdict={validation?.usSystem} local={localUsVerdict(draft, fields)} protocol={getPointer(draft, "/security/protocol/id")} />
        ) : null}
        {g.id === "group-scms" && movable.length > 0 ? (
          <div className="note setting-move top" data-testid="params-move-top">
            <span>
              {movable.length} lifecycle values come from this scenario's older parameter block.
            </span>
            <button type="button" className="btn small" onClick={() => moveIntoFields(draft, movable).forEach((w) => editSetting(w.pointer, w.value))}>
              Move them into the form
            </button>
          </div>
        ) : null}
        {subsOf(g, basic)}
        {advCount > 0 && basicCount > 0 ? (
          <button
            type="button"
            className={showAdv ? "adv-toggle on" : "adv-toggle"}
            aria-expanded={showAdv}
            data-testid="settings-advanced-toggle"
            onClick={() => toggleAdvanced(g.id, !showAdv)}
          >
            {showAdv ? "▾" : "▸"} Advanced <span className="count">{advCount}</span>
          </button>
        ) : null}
        {showAdv && advCount > 0 ? <div className="settings-advanced">{subsOf(g, (f) => !basic(f))}</div> : null}
        {g.id === "group-scms" && soon.length > 0 ? (
          <p className="faint coming-soon" data-testid="coming-soon">
            Coming soon: {soon.join(", ")}.
          </p>
        ) : null}
      </section>
    );
  };

  const runExtras = (
    <div className="run-extras">
      <dl className="kv compact">
        <dt>Scenario</dt>
        <dd>{hello?.scenarioName ?? "—"}</dd>
        <dt>Identity</dt>
        <dd>
          <Identifier value={scenarioHash} label="scenario digest" testId="scenario-hash" />
        </dd>
        {outputDigest ? (
          <>
            <dt>Result</dt>
            <dd title="A digest of everything the finished run produced. Two runs with the same settings and seed give the same one.">
              <Identifier value={outputDigest} label="output digest" testId="output-digest" />
            </dd>
          </>
        ) : null}
      </dl>
      {studies.length > 0 ? (
        <div className="study-chips" data-testid="studies">
          <span className="dim">Study:</span>
          {studies.map((st) => (
            <button
              key={st.id}
              type="button"
              className="chip"
              data-testid="study"
              data-study={st.id}
              disabled={busy !== null}
              title={`${st.question}\nSets ${st.patch.map((w) => w.path).join(", ")}`}
              onClick={() => {
                const s0 = useStudio.getState();
                const next = applyPatch(s0.draft ?? s0.scenario, st.patch);
                for (const w of st.patch) editSetting(w.pointer, getPointer(next, w.pointer));
                s0.setSettingsMessage({ text: `${st.label}: ${st.patch.length} settings set.${st.look_at ? ` Watch ${st.look_at}` : ""}`, tone: "info" });
              }}
            >
              {st.label}
            </button>
          ))}
        </div>
      ) : null}
      {presets.length > 0 ? (
        <details className="presets" data-testid="presets">
          <summary className="dim">Load a ready-made scenario ({presets.length})</summary>
          <div className="preset-grid">
            {presets.map((item) => (
              <div key={item.id} className="preset-row">
                <span title={item.description}>
                  {item.name ?? item.id}
                  {presets.filter((o) => o.name === item.name).length > 1 ? <span className="faint"> ({item.id.split("/").pop()})</span> : null}
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
        </details>
      ) : null}
    </div>
  );

  // What differs from the scenario the engine holds: each edit, by the field it is in.
  const changes = useMemo(
    () =>
      edits.map((p) => {
        const f = fields.filter((x) => p === x.pointer || p.startsWith(`${x.pointer}/`)).sort((a, b) => b.pointer.length - a.pointer.length)[0];
        const sec = f ? fullTree.find((g) => g.subs.some((s) => s.fields.includes(f)))?.name ?? researchSectionOf(f) : null;
        return { pointer: p, field: f, section: sec, from: getPointer(scenario, p), to: getPointer(draft, p) };
      }),
    [edits, fields, fullTree, scenario, draft],
  );

  const staged = extras.staged;

  return (
    <div
      className={detached ? "settings-window detached" : "settings-window"}
      data-testid="settings-window"
      onKeyDown={(e) => {
        if ((e.metaKey || e.ctrlKey) && !e.altKey && !e.shiftKey && e.key.toLowerCase() === "f") {
          e.preventDefault();
          search.current?.focus();
          search.current?.select();
        }
      }}
    >
      <div className="settings-head">
        <h2>Settings</h2>
        <div className="settings-search">
          <input
            ref={search}
            type="search"
            placeholder={`Search ${fields.length} settings`}
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              setView("form");
            }}
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
            {filtering ? `${found.length} found` : ""}
          </span>
        </div>
        <button
          type="button"
          className={view === "changes" ? "toggle on" : "toggle"}
          aria-pressed={view === "changes"}
          onClick={() => setView((v) => (v === "changes" ? "form" : "changes"))}
          data-testid="settings-changes"
          title="What differs from the scenario the engine holds"
        >
          Changes <span className="count">{edits.length}</span>
        </button>
        <div className="segmented" role="group" aria-label="View">
          <button type="button" className={view !== "json" ? "on" : ""} aria-pressed={view !== "json"} onClick={() => setView("form")} data-testid="settings-view-form">
            Form
          </button>
          <button type="button" className={view === "json" ? "on" : ""} aria-pressed={view === "json"} onClick={() => setView("json")} data-testid="settings-view-json">
            JSON
          </button>
        </div>
        {onDetach ? (
          <button type="button" className="icon-button" onClick={onDetach} data-testid="settings-detach" title="Open the settings in a window of their own" aria-label="Open in a new window">
            ⧉
          </button>
        ) : null}
        <button type="button" className="icon-button" onClick={onClose} data-testid="settings-close" title="Close the settings (Esc)" aria-label="Close the settings">
          ×
        </button>
      </div>

      <div className="settings-body">
        <nav
          className="settings-tree"
          aria-label="Setting sections"
          data-testid="settings-tree"
          onKeyDown={(e) => {
            if (e.key !== "ArrowDown" && e.key !== "ArrowUp" && e.key !== "Home" && e.key !== "End") return;
            const items = Array.from(e.currentTarget.querySelectorAll<HTMLElement>(".tree-item"));
            if (items.length === 0) return;
            e.preventDefault();
            const at = items.indexOf(document.activeElement as HTMLElement);
            const next = e.key === "Home" ? 0 : e.key === "End" ? items.length - 1 : e.key === "ArrowDown" ? Math.min(items.length - 1, at + 1) : Math.max(0, at - 1);
            items[next]?.focus();
          }}
        >
          {fullTree.map((g) => {
            const n = editedBySection.get(g.id) ?? 0;
            const hits = filtering ? foundTree.find((x) => x.id === g.id)?.count ?? 0 : null;
            return (
              <button
                key={g.id}
                type="button"
                className={`tree-item group${!filtering && view === "form" && current?.id === g.id ? " on" : ""}${hits === 0 ? " empty" : ""}`}
                onClick={() => {
                  setQuery("");
                  setModifiedOnly(false);
                  setView("form");
                  setActive(g.id);
                }}
                data-testid="settings-tree-group"
                data-group={g.name}
                title={g.blurb}
              >
                <span className="grow">{g.name}</span>
                {n > 0 ? <span className="edit-dot" title={`${n} unapplied edit${n === 1 ? "" : "s"}`} /> : null}
                <span className="count">{hits ?? g.count}</span>
              </button>
            );
          })}
          <div className="tree-filters">
            <button
              type="button"
              className={modifiedOnly ? "toggle on" : "toggle"}
              aria-pressed={modifiedOnly}
              onClick={() => {
                setModifiedOnly((v) => !v);
                setView("form");
              }}
              data-testid="settings-modified"
              title="Only settings that differ from the engine's default, and your edits"
            >
              Not default <span className="count">{modifiedCount}</span>
            </button>
            {unsupportedCount > 0 ? (
              <button
                type="button"
                className={showUnsupported ? "toggle on" : "toggle"}
                aria-pressed={showUnsupported}
                onClick={() => setShowUnsupported((v) => !v)}
                data-testid="settings-unsupported"
                title="Also show settings this engine accepts but does not act on"
              >
                Inactive <span className="count">{unsupportedCount}</span>
              </button>
            ) : null}
            <span className="faint source-line" data-testid="schema-source">
              {source === "published" ? `${fields.length} settings, each with what this engine does with it.` : source === "schema" ? "From the engine's schema." : "Standard fields; press Check to validate."}
            </span>
          </div>
        </nav>

        <div className="settings-content" ref={content} data-testid="scenario-panel">
          {validation && (!validation.valid || validation.warnings.length > 0) ? (
            <div className="settings-check">
              <span className={`pill ${validation.valid ? "ok" : "err"}`} data-testid="validation-state">
                {validation.valid ? "ready to run" : "needs fixing"}
              </span>
              <ProblemList
                items={validation.errors}
                onJump={jumpToField}
                nameOf={(p) => fields.filter((f) => p === f.pointer || p.startsWith(`${f.pointer}/`)).sort((a, b) => b.pointer.length - a.pointer.length)[0]?.label}
              />
              {validation.warnings.length > 0 ? (
                <div className="note" data-testid="validation-warn">
                  {validation.warnings.map((w, i) => (
                    <div key={i}>
                      <code>{w.path}</code> {w.message}
                    </div>
                  ))}
                </div>
              ) : null}
            </div>
          ) : validation ? (
            <span className="pill ok settings-ok" data-testid="validation-state">
              ready to run
            </span>
          ) : null}

          {view === "json" ? (
            <JsonView />
          ) : view === "changes" ? (
            <section className="settings-group" data-testid="settings-changes-list">
              <div className="section-head">
                <h3>Changes</h3>
                <span className="faint">What differs from the scenario the engine holds.</span>
              </div>
              {changes.length === 0 ? <p className="dim">No changes. Edit a setting and it is listed here.</p> : null}
              {changes.map((c) => (
                <div key={c.pointer} className="change-row" data-testid="change-row">
                  <button type="button" className="linkbutton change-name" onClick={() => jumpToField(c.pointer)} title={c.field ? fieldPath(c.field) : c.pointer}>
                    {c.section ? <span className="faint">{c.section} › </span> : null}
                    {c.field?.label ?? c.pointer}
                  </button>
                  <code className="change-from">{describeValue(c.from)}</code>
                  <span className="faint">→</span>
                  <code className="change-to">{describeValue(c.to)}</code>
                  <button type="button" className="icon-button tiny" onClick={() => undoEdit(c.pointer)} title="Undo this change" aria-label="Undo this change">
                    ↶
                  </button>
                </div>
              ))}
            </section>
          ) : filtering ? (
            foundTree.length === 0 ? (
              <p className="dim settings-empty" data-testid="settings-empty">
                No setting matches{query !== "" ? ` “${query}”` : ""}
                {modifiedOnly ? " among those not at their default" : ""}.
                {!showUnsupported && unsupportedCount > 0 ? " Inactive settings are hidden." : ""}
              </p>
            ) : (
              foundTree.map((g) => (
                <section key={g.id} className="settings-group" data-testid="settings-section" data-section-id={g.id}>
                  <div className="section-head">
                    <h3>{g.name}</h3>
                  </div>
                  {subsOf(g, () => true)}
                </section>
              ))
            )
          ) : current ? (
            sectionPage(current)
          ) : (
            <p className="dim settings-empty" data-testid="settings-empty">
              This engine published no settings.
            </p>
          )}
        </div>
      </div>

      <div className="settings-foot">
        <div className="settings-foot-text">
          {dirty ? (
            <div className="dim" data-testid="edit-route">
              {edits.length} unapplied change{edits.length === 1 ? "" : "s"}.
              {inert.length > 0 ? (
                <span className="err-text" data-testid="inert-edits">
                  {" "}
                  No effect in this build: {inert.map((f) => f.label).join(", ")}.
                </span>
              ) : null}
            </div>
          ) : null}
          {staged ? (
            <div className="dim" data-testid="staged-note">
              {staged.changed.length} applied setting{staged.changed.length === 1 ? "" : "s"} wait for the next run.{" "}
              <button type="button" className="linkbutton" disabled={busy !== null} data-testid="unstage" onClick={() => void runSettingsAction("Revert", revertStaged)}>
                Revert
              </button>
            </div>
          ) : null}
          {message ? (
            <div className={`settings-message ${message.tone === "err" ? "err-text" : "dim"}`} data-testid="scenario-message" role="status">
              {message.text}
              {message.tone !== "err" && message.text.startsWith(RESTART_NOTE) ? (
                <>
                  {" "}
                  <RestartNow onDone={() => useStudio.getState().setSettingsMessage(null)} />
                </>
              ) : null}
            </div>
          ) : null}
        </div>
        <button type="button" disabled={busy !== null} data-testid="validate" title="Ask the engine whether it accepts these settings" onClick={() => void runSettingsAction("Check", checkDraft)}>
          {busy === "Check" ? "Checking…" : "Check"}
        </button>
        <button type="button" disabled={busy !== null || !dirty} title="Drop every unapplied change" onClick={discardEdits} data-testid="discard-edits">
          Discard
        </button>
        <button type="button" disabled={busy !== null || !dirty} data-testid="apply" title="Send the changes to the engine; the next run uses them" onClick={() => void runSettingsAction("Apply", applyDraft)}>
          {busy === "Apply" ? "Applying…" : "Apply"}
        </button>
        <button
          type="button"
          className="primary"
          disabled={busy !== null}
          data-testid="run-start"
          title={dirty ? "Apply the changes and start a run" : "Start the scenario from the beginning"}
          onClick={() =>
            void runSettingsAction("Run", async () => {
              const said = await runWithDraft();
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
