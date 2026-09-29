/**
 * The Studio shell: the header, the viewport with the time controls under it, the inspector beside
 * it when there is something in it, and the panels that open over it
 * (docs/design/14-studio-shell.md).
 *
 * # Why it looks like this
 *
 * The page used to show five regions at once — a settings sidebar with four tabs, the viewport, an
 * inspector, a plots strip and a header — and gave the moving picture 37 % of a 1,280 x 800 window.
 * The owner found it cluttered, and it was: two of the five were empty most of the time, and the
 * settings column was too narrow to read a setting in. Now the viewport takes the page; the
 * settings are a window of their own (the gear, or Ctrl/Cmd + ,), the measurements a full-screen
 * panel (Metrics), the occasional panels are in the menu, and the inspector appears when you select
 * something or ask why a value is what it is.
 *
 * # The header
 *
 * What the run is doing, which scenario it is (and a menu to switch it), where the simulated clock
 * is, the one button worth pressing next, and the doors to everything else. The header stays over
 * the full-screen panels, so Pause is never hidden.
 *
 * # Saying what is happening
 *
 * Every state the connection and the run can be in gets a sentence and, where one helps, the
 * button that resolves it (`lib/status.ts`, `components/Status.tsx`), in the flow above the
 * viewport.
 *
 * # The engine target
 *
 * The Studio does not assume the fixture. On mount it resolves a base URL with `lib/target.ts`, and
 * Run details says which implementation answered: a plot of fixture values and a plot of
 * simulation results must not look the same.
 */

import { useCallback, useEffect, useState } from "react";

import { ComparePane } from "./components/ComparePane.js";
import { Inspector } from "./components/Inspector.js";
import { StatusBanner } from "./components/Status.js";
import { TimeControls } from "./components/TimeControls.js";
import { Viewport } from "./components/Viewport.js";
import { SettingsWindow } from "./settings/SettingsWindow.js";
import { Header } from "./shell/Header.js";
import { CloseIcon } from "./shell/Icons.js";
import { FullPanel, PANELS, Sheet } from "./shell/panels.js";
import { closePanel, togglePanel, usePanelRoute } from "./shell/route.js";
import { useThemeSync } from "./shell/theme.js";
import { applyThemeToDocument } from "./lib/theme.js";
import {
  candidateTargets,
  readOverride,
  resolveEngineTarget,
  worldFetchBlocked,
} from "./lib/target.js";
import { engine } from "./state/engine.js";
import { listenForOtherWindow } from "./state/settings.js";
import { useStudio, type PanelId } from "./state/store.js";

/** The settings window in a browser window of its own, keeping this page's engine choice. */
function detachSettings(): void {
  const params = new URLSearchParams(window.location.search);
  params.set("view", "settings");
  const url = `${window.location.pathname}?${params.toString()}`;
  const win = window.open(url, "vwp-settings", "popup,width=1180,height=860");
  if (win !== null) closePanel();
}

export function App(): React.JSX.Element {
  const theme = useStudio((s) => s.theme);
  const compare = useStudio((s) => s.compare);
  const panel = useStudio((s) => s.panel);
  const inspectorOpen = useStudio((s) => s.inspectorOpen);
  const setInspectorOpen = useStudio((s) => s.setInspectorOpen);
  const picking = useStudio((s) => s.mapPick);
  const [connectError, setConnectError] = useState<string | null>(null);
  // Panels that keep their body once opened, so a second look finds it as it was left.
  const [mounted, setMounted] = useState<ReadonlySet<PanelId>>(new Set());

  usePanelRoute();
  useThemeSync();

  useEffect(() => {
    if (panel !== null && PANELS[panel].keepMounted && !mounted.has(panel)) setMounted(new Set([...mounted, panel]));
  }, [panel, mounted]);

  /**
   * Resolve a target, then connect to it.
   *
   * Kept as one function because the two are one decision: connecting to an engine the app has not
   * identified is how the Studio ended up silently pinned to the fixture. The probe is cheap — one
   * `GET /healthz` per candidate, in sequence, with a short timeout — and its result is what Run
   * details reports.
   */
  const connectResolved = useCallback(async (): Promise<void> => {
    const storage = typeof localStorage === "undefined" ? null : localStorage;
    const override = readOverride(window.location.search, storage);
    const resolved = await resolveEngineTarget(candidateTargets(override), fetch);
    useStudio.getState().setTarget({
      baseUrl: resolved.baseUrl,
      flavour: resolved.probe?.flavour ?? "unknown",
      engine: resolved.probe?.engine ?? "no engine answered",
      reachable: resolved.probe?.reachable ?? false,
      pinned: resolved.pinned,
      tried: resolved.tried,
      worldBlocked: worldFetchBlocked(resolved.baseUrl, window.location.origin),
    });
    await engine.connect(resolved.baseUrl === "" ? window.location.origin : resolved.baseUrl);
    engine.attachViewer();
  }, []);

  // One resolve-and-connect on mount. StrictMode invokes this twice, and `engine.connect()`
  // coalesces the two into one attempt. It did not always: it only tore the previous client
  // down, which closed the first attempt's socket mid-handshake and surfaced as a 1006.
  useEffect(() => {
    let cancelled = false;
    void connectResolved()
      .then(() => {
        if (!cancelled) setConnectError(null);
      })
      .catch((err: unknown) => {
        if (!cancelled) setConnectError(err instanceof Error ? err.message : String(err));
      });
    return () => {
      cancelled = true;
    };
  }, [connectResolved]);

  // The settings window, opened in a window of its own, asks this page to start runs: this page
  // holds the stream a run is watched on. It also says when it changed what the engine holds.
  useEffect(() => listenForOtherWindow(() => engine.startRun()), []);

  const reconnect = useCallback(() => {
    setConnectError(null);
    void connectResolved().catch((err: unknown) => setConnectError(err instanceof Error ? err.message : String(err)));
  }, [connectResolved]);

  // The two keys that work anywhere: Ctrl/Cmd + , for the settings (VS Code's), Escape to close
  // the panel on top. A menu or a search box that uses Escape itself stops it before it gets here.
  useEffect(() => {
    const onKey = (ev: KeyboardEvent): void => {
      if ((ev.metaKey || ev.ctrlKey) && !ev.altKey && ev.key === ",") {
        ev.preventDefault();
        togglePanel("settings");
        return;
      }
      if (ev.key === "Escape" && !ev.defaultPrevented) {
        const state = useStudio.getState();
        if (state.mapPick !== null) {
          state.setMapPick(null);
          ev.preventDefault();
        } else if (state.panel !== null) {
          closePanel();
          ev.preventDefault();
        }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  useEffect(() => {
    applyThemeToDocument(theme);
  }, [theme]);

  const sheet = panel !== null && PANELS[panel].kind === "sheet" ? PANELS[panel] : null;

  return (
    <div className="app">
      <Header onConnect={reconnect} />

      <div className={inspectorOpen ? "body with-inspector" : "body"}>
        <main className="centre">
          <StatusBanner onConnect={reconnect} note={connectError} />
          <div className="stage">
            <div className={compare === null ? "viewstack" : "viewstack split"} data-testid="viewstack">
              <Viewport />
              {compare === null ? null : <ComparePane />}
            </div>
            {sheet !== null ? <Sheet key={sheet.id} spec={sheet} close={closePanel} retarget={reconnect} /> : null}
            {picking !== null ? (
              <div className="pick-banner" role="status" data-testid="map-pick-banner">
                <span>Click the map to pick a road for the {picking.purpose}.</span>
                <button type="button" onClick={() => useStudio.getState().setMapPick(null)}>
                  Cancel
                </button>
              </div>
            ) : null}
          </div>
          <TimeControls />
        </main>

        {inspectorOpen ? (
          <aside className="panel right inspector-dock" data-testid="inspector-panel" aria-label="Inspector">
            <button
              type="button"
              className="icon-button inspector-close"
              onClick={() => setInspectorOpen(false)}
              aria-label="Hide the inspector"
              title="Hide the inspector"
              data-testid="inspector-close"
            >
              <CloseIcon />
            </button>
            <Inspector />
          </aside>
        ) : null}
      </div>

      {panel === "settings" ? (
        // A map pick asked for from the settings (a closure's road) needs the map: the window steps
        // aside, keeping every half-typed value, until the click lands or is cancelled.
        <FullPanel id="settings" title="Settings" hidden={picking !== null}>
          <SettingsWindow onClose={closePanel} onDetach={detachSettings} />
        </FullPanel>
      ) : null}
      {(Object.values(PANELS) as (typeof PANELS)[PanelId][])
        .filter((p) => p.kind === "fullscreen" && p.id !== "settings" && (panel === p.id || (p.keepMounted && mounted.has(p.id))))
        .map((p) => (
          <FullPanel key={p.id} id={p.id} title={p.title} hidden={panel !== p.id}>
            {p.render({ close: closePanel, retarget: reconnect })}
          </FullPanel>
        ))}
    </div>
  );
}
