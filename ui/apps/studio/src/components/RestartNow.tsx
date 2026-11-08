/**
 * The button an applied edit's note carries: "Changes take effect when you restart — [Restart now]".
 *
 * A setting a run is built from (the map, the fleet, the duration…) cannot change under a run that
 * is moving, so an edit waits for the next run. Saying "the next run uses them — press Run" left
 * people looking for a Run button while a run was playing; this restarts in place instead, keeping
 * the settings, the way the Restart button in the time bar does.
 */
import { useState } from "react";
import { runWithDraft } from "../state/settings.js";
import { useStudio } from "../state/store.js";
import { describeError } from "../lib/errors.js";

export function RestartNow({ onDone }: { readonly onDone?: () => void }): React.JSX.Element {
  const [busy, setBusy] = useState(false);
  return (
    <button
      type="button"
      className="linklike restart-now"
      data-testid="restart-now"
      disabled={busy}
      onClick={() => {
        setBusy(true);
        void runWithDraft()
          .then(() => onDone?.())
          .catch((err: unknown) =>
            useStudio.getState().addLog({ at: Date.now(), level: "error", target: "run", message: describeError(err) }),
          )
          .finally(() => setBusy(false));
      }}
    >
      {busy ? "Restarting…" : "Restart now"}
    </button>
  );
}
