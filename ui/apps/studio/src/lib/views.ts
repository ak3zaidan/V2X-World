/**
 * The camera views in words: a name a person recognises and one sentence of what each shows.
 *
 * Kept out of the picker component so any part of the page that mentions a view (a hint, the
 * agent, the free camera's own help) says it the same way, and so the component file exports only
 * components (React fast refresh).
 */

import type { CameraMode } from "@vwp/viewer";

export const CAMERA_VIEWS: Readonly<Record<string, { readonly label: string; readonly hint: string }>> = {
  map: { label: "Map", hint: "The whole area from above. Click a vehicle to follow it." },
  chase: { label: "Chase", hint: "Ride just behind the selected vehicle." },
  dashboard: { label: "Driver's view", hint: "The road as the selected vehicle's driver sees it." },
  free: { label: "Free", hint: "Move the camera anywhere, not tied to a vehicle." },
  rsu: { label: "Roadside unit", hint: "Look out from a roadside radio mast." },
};

/** The name of a view. */
export function cameraLabel(mode: CameraMode): string {
  return CAMERA_VIEWS[mode]?.label ?? mode;
}
