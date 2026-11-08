/**
 * The free camera's controls, shown over the picture while it is the camera in use.
 *
 * The free camera used to have no keys at all — drag turned the view and nothing moved it — and
 * nothing on the page said so (the owner, 2026-10-06: "the free camera should be easier to control
 * and move around"). It now flies on the keys `@vwp/viewer` lists in `FREE_CAMERA_HINT`; this says
 * which, and that the view has to have the keyboard first. Styled inline so it needs nothing from
 * the shared stylesheet.
 */

import { useEffect, useState } from "react";
import { FREE_CAMERA_HINT } from "@vwp/viewer";

export function FreeCameraHint({ canvas }: { canvas: HTMLCanvasElement | null }): React.JSX.Element | null {
  const [focused, setFocused] = useState(() => canvas !== null && canvas.ownerDocument.activeElement === canvas);
  const [dismissed, setDismissed] = useState(false);

  useEffect(() => {
    if (!canvas) return;
    // Choosing "Free" from the camera menu leaves the keyboard on the menu: give it to the view,
    // so the first W moves the camera instead of doing nothing.
    canvas.focus({ preventScroll: true });
    setFocused(canvas.ownerDocument.activeElement === canvas);
    const on = (): void => setFocused(true);
    const off = (): void => setFocused(false);
    canvas.addEventListener("focus", on);
    canvas.addEventListener("blur", off);
    return () => {
      canvas.removeEventListener("focus", on);
      canvas.removeEventListener("blur", off);
    };
  }, [canvas]);

  if (dismissed) return null;
  return (
    <div
      data-testid="free-camera-hint"
      role="note"
      style={{
        position: "absolute",
        left: "50%",
        top: "calc(var(--vp-toolbar-bottom, 40px) + 8px)",
        transform: "translateX(-50%)",
        maxWidth: "calc(100% - 32px)",
        display: "flex",
        alignItems: "center",
        gap: 8,
        padding: "5px 6px 5px 10px",
        borderRadius: "var(--radius, 6px)",
        border: "1px solid var(--border)",
        background: "color-mix(in srgb, var(--bg-panel) 88%, transparent)",
        color: "var(--text-dim)",
        font: "12px var(--sans)",
        pointerEvents: "auto",
        zIndex: 3,
      }}
    >
      <span>
        {focused ? null : <strong style={{ color: "var(--text)" }}>Click the view, then: </strong>}
        {FREE_CAMERA_HINT}
      </span>
      <button
        type="button"
        onClick={() => setDismissed(true)}
        aria-label="Hide the free camera controls"
        title="Hide"
        style={{ border: "none", background: "transparent", color: "var(--text-dim)", cursor: "pointer", padding: "0 4px", font: "inherit" }}
      >
        ×
      </button>
    </div>
  );
}
