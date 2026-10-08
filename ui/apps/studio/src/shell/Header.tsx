/**
 * The header: what the run is doing, which scenario it is, where the simulated clock is, the one
 * button worth pressing next — and five doors: the metrics, the backend, the inspector, the settings, the menu.
 *
 * Everything else the page used to show at all times — four sidebar tabs, the engine build, the
 * digests, the frame counters — is one click away behind those doors (docs/design/14-studio-shell.md).
 */

import { useCallback, useEffect, useMemo, useState } from "react";

import { AgentBadge } from "../components/AgentBadge.js";
import { BackendButton } from "../components/BackendView.js";
import { ScenarioPicker } from "../components/ScenarioPicker.js";
import { QuickSettings } from "../settings/QuickSettings.js";
import { cityById, cityLabel, cityShortLabel, refreshCities, useCities } from "../state/cities.js";
import { HeaderClock } from "../components/HeaderClock.js";
import { PrimaryAction, StatusPill } from "../components/Status.js";
import { engine } from "../state/engine.js";
import { useStudio } from "../state/store.js";
import { transport } from "../state/transport.js";
import { applyThemeToDocument } from "../lib/theme.js";
import { changedPointers } from "../lib/schema.js";
import { RunSummaryButton } from "../metrics/RunSummary.js";
import { AgentIcon, ChartIcon, GearIcon, InspectorIcon, MoreIcon } from "./Icons.js";
import { MenuButton } from "./Menu.js";
import { PANELS } from "./panels.js";
import { openPanel, togglePanel } from "./route.js";

import "../styles/settings-widgets.css";
import "../styles/picker.css";
import "../styles/quick.css";

const MOD = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform) ? "⌘" : "Ctrl+";

/**
 * The scenario's name and the city it runs on. Pressing it opens the picker: a ready-made scenario
 * first, then a city of the engine's library, then Run (`components/ScenarioPicker.tsx`).
 */
function ScenarioSwitcher(): React.JSX.Element {
  const name = useStudio((s) => s.hello?.scenarioName ?? null);
  const runningHash = useStudio((s) => s.scenarioExtras.runningHash);
  const cities = useCities((s) => s.cities);
  const current = useCities((s) => s.current);
  const [open, setOpen] = useState(false);
  // The city follows the run: asked again whenever a different scenario starts running.
  useEffect(() => {
    void refreshCities();
  }, [runningHash, name]);
  const city = cityById(cities, current);
  return (
    <>
      <button
        type="button"
        className="scenario-switch"
        title={name ? `Scenario ${name}${city ? ` on ${cityLabel(city)}` : ""}: choose another scenario or city` : "Choose a scenario and a city"}
        data-testid="scenario-switcher"
        aria-haspopup="dialog"
        onClick={() => setOpen(true)}
      >
        <span className="topbar-scenario" data-testid="header-scenario">
          {name ?? <span className="faint">no scenario loaded</span>}
        </span>
        {city ? (
          <span className="topbar-city" data-testid="header-city" title={`${city.name}, ${city.state_name}: ${city.description}`}>
            {cityShortLabel(city)}
          </span>
        ) : null}
        <span className="caret" aria-hidden="true">▾</span>
      </button>
      {open ? <ScenarioPicker onClose={() => setOpen(false)} /> : null}
    </>
  );
}

/**
 * Restart and Stop: the two run controls that left the transport bar, which keeps play, step, speed
 * and the timeline. They are the same calls with the same rules (`state/transport.ts`), so a
 * disabled item says why exactly as the bar's buttons did.
 */
function RunItems({ close }: { close: () => void }): React.JSX.Element | null {
  const connection = useStudio((s) => s.connection);
  const run = useStudio((s) => s.run);
  const replay = useStudio((s) => s.replay);
  const hello = useStudio((s) => s.hello);
  const simTimeNs = useStudio((s) => s.simTimeNs);
  if (replay !== null) return null;
  const caps = transport({
    connection,
    runState: run.state,
    tNs: run.tNs,
    tEndNs: run.tEndNs > 0 ? run.tEndNs : hello?.simDurationNs ?? 0,
    streamNs: simTimeNs > 0 ? simTimeNs : run.tNs,
    recording: null,
    busy: false,
  });
  const act = (fn: () => Promise<unknown>): void => {
    close();
    void fn()
      .catch((err: unknown) => useStudio.getState().addLog({ at: Date.now(), level: "error", target: "run", message: err instanceof Error ? err.message : String(err) }))
      .finally(() => void engine.refreshStatus());
  };
  return (
    <>
      <button
        type="button"
        role="menuitem"
        className="menu-item"
        disabled={!caps.restart.enabled}
        title={caps.restart.why}
        data-testid="restart"
        onClick={() => act(() => engine.startRun())}
      >
        <span className="grow">Restart from the beginning</span>
      </button>
      <button
        type="button"
        role="menuitem"
        className="menu-item"
        disabled={!caps.stop.enabled}
        title={caps.stop.why}
        data-testid="stop"
        onClick={() => act(() => engine.request("run.stop", {}))}
      >
        <span className="grow">Stop the run</span>
      </button>
    </>
  );
}

/** The menu: the run's restart and stop, the occasional panels, the theme, developer mode. */
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
          <RunItems close={close} />
          <div className="menu-rule" />
          {item(
            "runs",
            "Runs and recordings",
            "Open a recording from a file, or look at the run on screen",
            close,
            recordings > 0 ? <span className="count">{recordings}</span> : null,
          )}
          {item("compare", "Compare two runs", "Put a second run or a recording beside this one, on one clock", close, compare !== null ? <span className="count">B</span> : null)}
          {item("details", "Run details", "Which engine and build ran this, its timing, and the stream counters", close, null, "run-details-button")}
          {/* The engine's raw command catalogue is for whoever is developing or scripting it. */}
          {devDetails ? item("commands", "Engine commands", "Every command this engine accepts, and every one this page has sent", close) : null}
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
            <span className="check-mark" aria-hidden="true" />
            <span className="grow">{theme === "dark" ? "Light theme" : "Dark theme"}</span>
          </button>
          <button
            type="button"
            role="menuitemcheckbox"
            aria-checked={devDetails}
            className="menu-item"
            data-testid="dev-details-toggle"
            onClick={() => useStudio.getState().setDevDetails(!devDetails)}
            title="Show frame rate, draw calls and the map build report over the scene, the engine's command list, and protocol names and specification references"
          >
            <span className="check-mark" aria-hidden="true">{devDetails ? "✓" : ""}</span>
            <span className="grow">Developer mode</span>
          </button>
          <div className="menu-rule" />
          <div className="menu-note faint shortcuts" aria-label="Keyboard shortcuts">
            <span>Metrics</span>
            <kbd>M</kbd>
            <span>Settings</span>
            <kbd>{MOD},</kbd>
            <span>Search the settings</span>
            <kbd>{MOD}F</kbd>
            <span>Switch camera view</span>
            <kbd>[ ]</kbd>
            <span>Frame the traffic</span>
            <kbd>F</kbd>
            <span>Next or previous event</span>
            <kbd>Alt ← →</kbd>
            <span>Close a panel</span>
            <kbd>Esc</kbd>
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
  const [quickOpen, setQuickOpen] = useState(false);
  const draft = useStudio((s) => s.draft);
  const scenario = useStudio((s) => s.scenario);
  // Computed on a change of either document, not on every store beat (the store flushes at 5 Hz).
  const edits = useMemo(() => (draft === null ? 0 : changedPointers(scenario, draft).length), [draft, scenario]);
  const pending = edits > 0 || staged;
  const coversTimebar = panel !== null && PANELS[panel].kind === "fullscreen";

  return (
    <header className="topbar">
      <h1 title="V2X World Simulator">
        V2X<span className="brand-sub">Simulator</span>
      </h1>
      <StatusPill />
      <ScenarioSwitcher />
      {/* The transport bar under the scene shows the clock; a full-screen panel covers that bar,
          so the header carries it then and only then — two clocks a few hundred pixels apart said
          the same thing twice. */}
      {coversTimebar ? <HeaderClock /> : null}
      <span className="grow" />
      <PrimaryAction onConnect={onConnect} />
      <span className="topbar-rule" aria-hidden="true" />
      <button
        type="button"
        className={panel === "metrics" ? "icon-button labelled on" : "icon-button labelled"}
        aria-pressed={panel === "metrics"}
        onClick={() => togglePanel("metrics")}
        data-testid="metrics-button"
        title="Charts of every measurement in this run, and whether each is in a healthy range (M)"
      >
        <ChartIcon />
        <span>Metrics</span>
      </button>
      <RunSummaryButton />
      <button
        type="button"
        className={panel === "agent" ? "icon-button labelled on" : "icon-button labelled"}
        aria-pressed={panel === "agent"}
        onClick={() => togglePanel("agent")}
        data-testid="agent-button"
        title="The agent watches every run and sums it up; ask it to set up, run or explain a simulation"
      >
        <AgentIcon />
        <span>Agent</span>
        <AgentBadge />
      </button>
      <BackendButton />
      <button
        type="button"
        className={inspectorOpen ? "icon-button on" : "icon-button"}
        aria-pressed={inspectorOpen}
        aria-label="Vehicle details"
        onClick={() => setInspectorOpen(!inspectorOpen)}
        data-testid="inspector-toggle"
        title={inspectorOpen ? "Hide the details (I)" : "Details of the selected vehicle or roadside unit: messages sent and received, and queues (I)"}
      >
        <InspectorIcon />
      </button>
      <button
        type="button"
        className={quickOpen ? "icon-button labelled on" : "icon-button labelled"}
        aria-pressed={quickOpen}
        aria-expanded={quickOpen}
        onClick={() => setQuickOpen((v) => !v)}
        data-testid="quick-button"
        title="The settings changed most: traffic, radio, message rate, security, attackers, run length, seed, files"
      >
        <span>Quick settings</span>
      </button>
      {quickOpen ? (
        <QuickSettings
          onClose={() => setQuickOpen(false)}
          onOpenSettings={() => {
            setQuickOpen(false);
            openPanel("settings");
          }}
        />
      ) : null}
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
