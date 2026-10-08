/**
 * How a value's health is shown: a coloured dot and level, the value, and one line saying what it
 * means ("0.71 — above the 0.68 ETSI congestion target; vehicles will throttle their messages"). The
 * thresholds and their source are in the tooltip and, in the expanded view, written out.
 *
 * Colour is never the only signal: the level is always written beside the dot.
 */

import type { MetricHealthBand } from "@vwp/protocol";

import { BASIS_LABEL, LEVEL_LABEL, classify, judgedLabel, noteOf, thresholdsText, type HealthLevel } from "./health.js";
import { formatWithUnit } from "./model.js";

export function HealthDot({ level }: { level: HealthLevel }): React.JSX.Element {
  return <span className={`hdot ${level}`} aria-hidden="true" />;
}

/**
 * A level as a small coloured chip ("● Unhealthy"); what it means, the thresholds and their source
 * are its tooltip. The card and the expanded view's headline both use it.
 */
export function HealthChip({
  band,
  level,
  unit,
  testId,
  large = false,
}: {
  band: MetricHealthBand;
  level: HealthLevel;
  unit: string;
  testId?: string;
  large?: boolean;
}): React.JSX.Element {
  return (
    <span
      className={`hchip ${level}${large ? " large" : ""}`}
      data-testid={testId}
      data-level={level}
      title={`${noteOf(band, level)}\n${thresholdsText(band, unit)}\n${BASIS_LABEL[band.basis]}: ${band.source}`}
    >
      <HealthDot level={level} />
      {LEVEL_LABEL[level]}
    </span>
  );
}

/** A value judged against its band, in one line. Nothing when the value is missing. */
export function HealthLine({
  band,
  value,
  unit,
  testId,
  compact = false,
}: {
  band: MetricHealthBand;
  value: number | null | undefined;
  unit: string;
  testId?: string;
  compact?: boolean;
}): React.JSX.Element | null {
  const level = classify(band, value);
  if (level === null || value === null || value === undefined) return null;
  const stat = judgedLabel(band);
  const title = `${thresholdsText(band, unit)}\n${BASIS_LABEL[band.basis]}: ${band.source}`;
  return (
    <div className={`hline ${level}${compact ? " compact" : ""}`} title={title} data-testid={testId} data-level={level}>
      <HealthDot level={level} />
      <span className="hline-level">{LEVEL_LABEL[level]}</span>
      <span className="hline-text">
        {stat !== "" ? <span className="dim">{stat} </span> : null}
        <strong className="mono">{formatWithUnit(value, unit)}</strong> — {noteOf(band, level)}
      </span>
    </div>
  );
}

/** The band written out: its three levels with their thresholds, and where the numbers come from. */
export function BandTable({ band, unit }: { band: MetricHealthBand; unit: string }): React.JSX.Element {
  const u = unit === "ratio" || unit === "1" || unit === "-" ? "" : ` ${unit}`;
  const op = band.better === "lower" ? ["≤", "≤", ">"] : ["≥", "≥", "<"];
  const stat = judgedLabel(band);
  return (
    <div className="hband" data-testid="health-band">
      <h4>Health thresholds{stat !== "" ? <span className="dim"> · judged {stat.startsWith("within ") ? stat : `on the ${stat}`}</span> : null}</h4>
      <ul>
        <li className="healthy">
          <HealthDot level="healthy" /> <strong>Healthy</strong> {op[0]} {band.caution}
          {u} — {band.healthy_note}
        </li>
        <li className="caution">
          <HealthDot level="caution" /> <strong>Caution</strong> {op[1]} {band.unhealthy}
          {u} — {band.caution_note}
        </li>
        <li className="unhealthy">
          <HealthDot level="unhealthy" /> <strong>Unhealthy</strong> {op[2]} {band.unhealthy}
          {u} — {band.unhealthy_note}
        </li>
      </ul>
      <p className="dim hband-src">
        {BASIS_LABEL[band.basis]}. Source: {band.source}
      </p>
    </div>
  );
}
