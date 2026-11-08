/**
 * The settings window in a browser window of its own (`?view=settings`).
 *
 * It reads and writes the scenario over HTTP only — no stream, no WebGL — because it never draws a
 * frame. It resolves the engine the same way the main page does (so `?engine=` and a pinned engine
 * are honoured), polls the run's status for the staged note, and follows the main page's changes
 * over the settings channel (`state/settings.ts`). Run is handed to the main page, which holds the
 * stream the run is watched on.
 */

import { useEffect, useState } from "react";

import { candidateTargets, readOverride, resolveEngineTarget } from "../lib/target.js";
import { engine } from "../state/engine.js";
import { listenForOtherWindow } from "../state/settings.js";
import { useThemeSync } from "../shell/theme.js";
import { SettingsWindow } from "./SettingsWindow.js";

export function SettingsStandalone(): React.JSX.Element {
  const [error, setError] = useState<string | null>(null);
  useThemeSync();

  useEffect(() => {
    document.title = "Settings — V2X Simulator";
    let timer: ReturnType<typeof setInterval> | null = null;
    const storage = typeof localStorage === "undefined" ? null : localStorage;
    void resolveEngineTarget(candidateTargets(readOverride(window.location.search, storage)), fetch)
      .then(async (resolved) => {
        engine.attachHttpOnly(resolved.baseUrl === "" ? window.location.origin : resolved.baseUrl);
        if (!resolved.probe?.reachable) setError("No engine answered. The settings shown may be out of date.");
        await engine.refreshScenario();
        await engine.refreshStatus();
        timer = setInterval(() => void engine.refreshStatus(), 2000);
      })
      .catch((err: unknown) => setError(err instanceof Error ? err.message : String(err)));
    const stop = listenForOtherWindow();
    return () => {
      if (timer !== null) clearInterval(timer);
      stop();
    };
  }, []);

  return (
    <div className="app standalone">
      {error ? (
        <div className="statusbar warn" role="status">
          <div className="statusbar-text">{error}</div>
        </div>
      ) : null}
      <SettingsWindow detached onClose={() => window.close()} />
    </div>
  );
}
