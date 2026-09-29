/**
 * One theme across the page's windows: the choice is remembered per viewer (`vwp.studio.theme`), and
 * a change made in one window — the settings opened in a window of their own, say — reaches the
 * other through the storage event, so the two never disagree about which palette they are in.
 */

import { useEffect } from "react";

import { applyThemeToDocument } from "../lib/theme.js";
import { engine } from "../state/engine.js";
import { readTheme, useStudio } from "../state/store.js";

export function useThemeSync(): void {
  useEffect(() => {
    const onStorage = (ev: StorageEvent): void => {
      if (ev.key !== "vwp.studio.theme") return;
      const next = readTheme();
      if (useStudio.getState().theme === next) return;
      useStudio.getState().setTheme(next);
      applyThemeToDocument(next);
      engine.viewer?.setTheme(next);
    };
    window.addEventListener("storage", onStorage);
    return () => window.removeEventListener("storage", onStorage);
  }, []);
}
