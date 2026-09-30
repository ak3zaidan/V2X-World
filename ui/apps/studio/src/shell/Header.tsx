/**
 * The header: what the run is doing, which scenario it is, where the simulated clock is, the one
 * button worth pressing next — and four doors: the metrics, the inspector, the settings, the menu.
 *
 * Everything else the page used to show at all times — four sidebar tabs, the engine build, the
 * digests, the frame counters — is one click away behind those doors (docs/design/14-studio-shell.md).
 */

import { useCallback, useMemo, useState } from "react";

import { HeaderClock } from "../components/HeaderClock.js";
import { PrimaryAction, StatusPill } from "../components/Status.js";
import { engine } from "../state/engine.js";
import { presetsOf, loadPreset } from "../state/settings.js";
import { useStudio } from "../state/store.js";
import { applyThemeToDocument } from "../lib/theme.js";
import { changedPointers } from "../lib/schema.js";
import { ChartIcon, GearIcon, InspectorIcon, MoreIcon } from "./Icons.js";
import { MenuButton } from "./Menu.js";
import { openPanel, togglePanel } from "./route.js";

const MOD = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform) ? "⌘" : "Ctrl+";

/** The scenario's name, which opens the list of ready-made scenarios and the settings. */
function ScenarioSwitcher(): React.JSX.Element {
  const name = useStudio((s) => s.hello?.scenarioName ?? null);
  const list = useStudio((s) => s.scenarioList);
  const runningHash = useStudio((s) => s.scenarioExtras.runningHash);
  const [note, setNote] = useState<{ text: string; err: boolean } | null>(null);
  const [busy, setBusy] = useState(false);
  const presets = presetsOf(list);

  return (
    <MenuButton
      className="scenario-switch"
      title={name ? `Scenario ${name}: switch to another, or change its settings` : "Choose a scenario"}
      testId="scenario-switcher"
      menuTestId="scenario-menu"
      align="left"
      label={
        <>
          <span className="topbar-scenario" data-testid="header-scenario">
            {name ?? <span className="faint">no scenario loaded</span>}
          </span>
          <span className="caret" aria-hidden="true">▾</span>
        </>
      }
    >
      {(close) => (
        <>
          <div className="sec">Ready-made scenarios</div>
          {presets.length === 0 ? <div className="dim menu-note">This engine lists none.</div> : null}
          {presets.map((item) => (
            <button
              key={item.id}
              type="button"
              role="menuitem"
              className="menu-item"
              disabled={busy}
              data-testid="switcher-load"
              data-preset={item.name ?? item.id}
              title={item.description ?? "Load these settings; the next run uses them"}
              onClick={() => {
                setBusy(true);
                setNote(null);
                loadPreset(item)
                  .then((text) => setNote({ text, err: false }))
                  .catch((err: unknown) => setNote({ text: err instanceof Error ? err.message : String(err), err: true }))
                  .finally(() => setBusy(false));
              }}
            >
              <span className="grow">{item.name ?? item.id}</span>
              {item.hash !== undefined && item.hash === runningHash ? <span className="faint">running</span> : null}
            </button>
          ))}
          {note ? (
            <div className={note.err ? "menu-note err-text" : "menu-note"} data-testid="switcher-message" role="status">
              {note.text}
            </div>
          ) : null}
          <div className="menu-rule" />
          <button
            type="button"
            role="menuitem"
            className="menu-item"
            onClick={() => {
              close();
              openPanel("settings");
            }}
          >
            <span className="grow">Edit settings…</span>
            <kbd>{MOD},</kbd>
          </button>
        </>
      )}
    </MenuButton>
  );
}

/** The menu: the occasional panels, the theme, developer mode. */
function AppMenu(): React.JSX.Element {
  const theme = useStudio((s) => s.theme);
  const devDetails = useStudio((s) => s.devDetails);
  const compare = useStudio((s) => s.compare);
  const recordings = useStudio((s) => s.recordings.length);

  const toggleTheme = useCallback(() => {
    const next = useStudio.getState().theme === "dark" ? "light" : "dark";
    useStudio.getState().setTheme(next);
    applyThemeToDocument(next);
    engine.viewer?.setTheme(next);
  }, []);

  const item = (id: "runs" | "compare" | "commands" | "details", label: string, hint: string, close: () => void, extra?: React.ReactNode, testId?: string): React.JSX.Element => (
    <button
      type="button"
      role="menuitem"
      className="menu-item"
      title={hint}
      data-testid={testId ?? `menu-${id}`}
      onClick={() => {
        close();
        openPanel(id);
      }}
    >
      <span className="grow">{label}</span>
      {extra}
    </button>
  );

  return (
    <MenuButton className="icon-button" title="More" testId="app-menu-button" menuTestId="app-menu" label={<MoreIcon />}>
      {(close) => (
        <>
          {item(
            "runs",
            "Runs and recordings",
            "The run on screen, recordings opened from files, and the world under them",
            close,
            recordings > 0 ? <span className="count">{recordings}</span> : null,
          )}
          {item("compare", "Compare two runs", "Put a second run or a recording beside this one, on one clock", close, compare !== null ? <span className="count">B</span> : null)}
          {item("commands", "Commands", "Every command this engine accepts, and every one this page has sent", close)}
          {item("details", "Run details", "Run identity, engine build, timing and stream counters", close, null, "run-details-button")}
          <div className="menu-rule" />
          <button
            type="button"
            role="menuitem"
            className="menu-item"
            data-testid="theme-toggle"
            onClick={() => {
              toggleTheme();
            }}
            title="Switch between the dark and light palette"
          >
            <span className="grow">{theme === "dark" ? "Light theme" : "Dark theme"}</span>
          </button>
          <button
            type="button"
            role="menuitemcheckbox"
            aria-checked={devDetails}
            className="menu-item"
            data-testid="dev-details-toggle"
            onClick={() => useStudio.getState().setDevDetails(!devDetails)}
            title="Show frame rate, draw calls and the world build report over the viewport, and protocol names and specification references in the explanations"
          >
            <span className="check-mark" aria-hidden="true">{devDetails ? "✓" : ""}</span>
            <span className="grow">Developer mode</span>
          </button>
          <div className="menu-rule" />
          <div className="menu-note faint shortcuts" aria-label="Keyboard shortcuts">
            <span>Settings</span>
            <kbd>{MOD},</kbd>
            <span>Search the settings</span>
            <kbd>{MOD}F</kbd>
            <span>Close a panel</span>
            <kbd>Esc</kbd>
            <span>Camera mode</span>
            <kbd>[ ]</kbd>
          </div>
        </>
      )}
    </MenuButton>
  );
}

export function Header({ onConnect }: { onConnect: () => void }): React.JSX.Element {
  const panel = useStudio((s) => s.panel);
  const inspectorOpen = useStudio((s) => s.inspectorOpen);
  const setInspectorOpen = useStudio((s) => s.setInspectorOpen);
  const staged = useStudio((s) => s.scenarioExtras.staged !== null);
  const draft = useStudio((s) => s.draft);
  const scenario = useStudio((s) => s.scenario);
  // Computed on a change of either document, not on every store beat (the store flushes at 5 Hz).
  const edits = useMemo(() => (draft === null ? 0 : changedPointers(scenario, draft).length), [draft, scenario]);
  const pending = edits > 0 || staged;

  return (
    <header className="topbar">
      <h1 title="V2X World Simulator">V2X</h1>
      <StatusPill />
      <ScenarioSwitcher />
      <HeaderClock />
      <span className="grow" />
      <PrimaryAction onConnect={onConnect} />
      <span className="topbar-rule" aria-hidden="true" />
      <button
        type="button"
        className={panel === "metrics" ? "icon-button labelled on" : "icon-button labelled"}
        aria-pressed={panel === "metrics"}
        onClick={() => togglePanel("metrics")}
        data-testid="metrics-button"
        title="Measurements, full screen"
      >
        <ChartIcon />
        <span>Metrics</span>
      </button>
      <button
        type="button"
        className={inspectorOpen ? "icon-button on" : "icon-button"}
        aria-pressed={inspectorOpen}
        aria-label="Inspector"
        onClick={() => setInspectorOpen(!inspectorOpen)}
        data-testid="inspector-toggle"
        title={inspectorOpen ? "Hide the inspector" : "Show the inspector: the selected radio, why a value is what it is, and the log"}
      >
        <InspectorIcon />
      </button>
      <button
        type="button"
        className={panel === "settings" ? "icon-button on" : "icon-button"}
        aria-pressed={panel === "settings"}
        aria-label={pending ? `Settings (${edits > 0 ? `${edits} unapplied edit${edits === 1 ? "" : "s"}` : "applied settings wait for the next run"})` : "Settings"}
        onClick={() => togglePanel("settings")}
        data-testid="settings-button"
        title={`Settings (${MOD},)${edits > 0 ? ` — ${edits} unapplied edit${edits === 1 ? "" : "s"}` : staged ? " — applied settings wait for the next run" : ""}`}
      >
        <GearIcon />
        {pending ? <span className="badge-dot" data-testid="settings-pending" aria-hidden="true" /> : null}
      </button>
      <AppMenu />
    </header>
  );
}
