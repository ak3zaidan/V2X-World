/**
 * How a value's health is shown: a coloured dot and level, the value, and one line saying what it
 * means ("0.71 — above the 0.68 ETSI congestion target; vehicles will throttle their messages"). The
 * thresholds and their source are in the tooltip and, in the expanded view, written out.
 *
 * Colour is never the only signal: the level is always written beside the dot.
 */

import type { MetricHealthBand } from "@vwp/protocol";

import { BASIS_LABEL, LEVEL_LABEL, bandValue, classify, judgedLabel, noteOf, thresholdsText, type HealthLevel } from "./health.js";

/**
 * The level's mark: a shape as well as a colour, so it reads in grey and to a colour-blind reader —
 * a circle for healthy, a triangle for caution, a diamond for unhealthy (`metrics.css` `.hdot`).
 */
export function HealthDot({ level }: { level: HealthLevel }): React.JSX.Element {
  return <span className={`hdot ${level}`} aria-hidden="true" />;
}

/**
 * The mark and its level's word ("▲ Caution"), the same three words the cards, the health strip and
 * the run summary use; `compact` is the mark alone, labelled for a screen reader.
 */
export function LevelMark({ level, compact = false, title }: { level: HealthLevel; compact?: boolean; title?: string }): React.JSX.Element {
  return (
    <span className={`lmark ${level}${compact ? " compact" : ""}`} data-level={level} title={title ?? LEVEL_LABEL[level]} role={compact ? "img" : undefined} aria-label={compact ? `${LEVEL_LABEL[level]}${title ? `: ${title}` : ""}` : undefined}>
      <HealthDot level={level} />
      {compact ? null : LEVEL_LABEL[level]}
    </span>
  );
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
        <strong className="mono">{bandValue(value, unit)}</strong> — {noteOf(band, level)}
      </span>
    </div>
  );
}

/** The band written out: its three levels with their thresholds, and where the numbers come from. */
export function BandTable({ band, unit }: { band: MetricHealthBand; unit: string }): React.JSX.Element {
  const op = band.better === "lower" ? ["≤", "≤", ">"] : ["≥", "≥", "<"];
  const stat = judgedLabel(band);
  return (
    <div className="hband" data-testid="health-band">
      <h4>Health thresholds{stat !== "" ? <span className="dim"> · judged {stat.startsWith("within ") ? stat : `on the ${stat}`}</span> : null}</h4>
      <ul>
        <li className="healthy">
          <HealthDot level="healthy" /> <strong>Healthy</strong> {op[0]} {bandValue(band.caution, unit)} — {band.healthy_note}
        </li>
        <li className="caution">
          <HealthDot level="caution" /> <strong>Caution</strong> {op[1]} {bandValue(band.unhealthy, unit)} — {band.caution_note}
        </li>
        <li className="unhealthy">
          <HealthDot level="unhealthy" /> <strong>Unhealthy</strong> {op[2]} {bandValue(band.unhealthy, unit)} — {band.unhealthy_note}
        </li>
      </ul>
      <p className="dim hband-src">
        {BASIS_LABEL[band.basis]}. Source: {band.source}
      </p>
    </div>
  );
}
