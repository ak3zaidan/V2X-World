/**
 * What to call a followed actor in the viewport chrome: "pedestrian", "cyclist", "bus" — the class
 * name from the Hello's class table — rather than "vehicle" for everything, which put "Following
 * vehicle 1" over a person walking down the sidewalk.
 */

import { engine } from "../state/engine.js";

/** The class name of actor `id` in the live stream, lower case, or "vehicle" when unknown. */
export function actorNoun(id: number): string {
  const poses = engine.client?.poses;
  const classes = engine.viewer?.actors.classes;
  if (!poses || !classes) return "vehicle";
  for (let slot = 0; slot < poses.count; slot++) {
    if (poses.occupied[slot] !== 1 || poses.actorId[slot] !== id >>> 0) continue;
    const c = classes[poses.classIdx[slot]];
    if (!c) return "vehicle";
    // Vehicle classes are named by body ("passenger", "truck"); only a road user that is not a
    // vehicle (category 1, a person or a rider) or a bus reads better by its own name.
    const name = c.name.replace(/_/g, " ").toLowerCase();
    if (c.category === 1 || name === "bus" || name === "tram") return name;
    return "vehicle";
  }
  return "vehicle";
}
