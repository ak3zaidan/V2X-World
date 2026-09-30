/**
 * The actor-state legend (09-ui §10): a colour-blind-safe palette with shape redundancy.
 *
 * Both the colour and the shape come from `@vwp/viewer` — the colours are its Okabe–Ito
 * `theme.actorState` entries and the shapes are the marker geometries `StateMarkerOverlay` draws —
 * so the key in the DOM is the key to the scene, not an approximation of it.
 */

import { VRU_MARK_SCALE } from "@vwp/viewer";

import { useStudio } from "../state/store.js";
import { actorStatePalette } from "../lib/theme.js";

/**
 * The road-user key: at map altitude every actor is a dot in its state colour, and a pedestrian's
 * or a cyclist's dot is {@link VRU_MARK_SCALE} the size of a vehicle's (`ActorLocatorOverlay`).
 * Up close each is drawn as itself — a car, a person, a rider on a bicycle.
 */
function RoadUserKey({ color }: { color: string }): React.JSX.Element {
  const r = 4;
  return (
    <>
      <span className="item" data-testid="legend-vehicle">
        <svg width="12" height="12" viewBox="-6 -6 12 12" aria-hidden="true" focusable="false">
          <circle r={r} fill={color} />
        </svg>
        vehicle
      </span>
      <span className="item" data-testid="legend-vru">
        <svg width="12" height="12" viewBox="-6 -6 12 12" aria-hidden="true" focusable="false">
          <circle r={r * VRU_MARK_SCALE} fill={color} />
        </svg>
        pedestrian / cyclist
      </span>
    </>
  );
}

function Glyph({ shape, color }: { shape: string; color: string }): React.JSX.Element {
  const common = { fill: color, stroke: color, strokeWidth: 1.4 } as const;
  return (
    <svg width="12" height="12" viewBox="-6 -6 12 12" aria-hidden="true" focusable="false">
      {shape === "circle" ? <circle r="4" {...common} /> : null}
      {shape === "triangle" ? <polygon points="0,-5 4.5,4 -4.5,4" {...common} /> : null}
      {shape === "diamond" ? <polygon points="0,-5 5,0 0,5 -5,0" {...common} /> : null}
      {shape === "cross" ? (
        <path d="M-4.5,-4.5 L4.5,4.5 M4.5,-4.5 L-4.5,4.5" fill="none" stroke={color} strokeWidth="2.2" />
      ) : null}
      {shape === "ring" ? <circle r="4" fill="none" stroke={color} strokeWidth="2" /> : null}
    </svg>
  );
}

/**
 * The key, folded to one chip at the viewport's top right until it is asked for, and remembered
 * open or shut across a reload. It used to be a column of eight rows over the picture at all times;
 * a researcher needs it the first few minutes and then not at all.
 */
export function StateLegend(): React.JSX.Element {
  const theme = useStudio((s) => s.theme);
  const open = useStudio((s) => s.legendOpen);
  const setOpen = useStudio((s) => s.setLegendOpen);
  const palette = actorStatePalette(theme);
  return (
    <div className="vp-legend">
      <button
        type="button"
        className={open ? "chip vp-legend-toggle on" : "chip vp-legend-toggle"}
        aria-expanded={open}
        aria-controls="vp-legend-body"
        onClick={() => setOpen(!open)}
        data-testid="legend-toggle"
        title={open ? "Hide the key" : "What the colours and shapes mean"}
      >
        <span className="vp-legend-swatches" aria-hidden="true">
          {palette.slice(0, 4).map((p) => (
            <i key={p.key} style={{ background: p.color }} />
          ))}
        </span>
        key
      </button>
      {open ? (
        <div className="chip legend vp-legend-body" id="vp-legend-body" data-testid="state-legend">
          {palette.map((p) => (
            <span className="item" key={p.key}>
              <Glyph shape={p.shape} color={p.color} />
              {p.label}
            </span>
          ))}
          <RoadUserKey color={palette[0]?.color ?? "currentColor"} />
        </div>
      ) : null}
    </div>
  );
}
