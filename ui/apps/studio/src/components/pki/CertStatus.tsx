/**
 * The one visual language for a certificate's state (`lib/pkiCert.ts` `CERT_STATUS`): a shape in
 * the state's colour, so it reads without colour, with its label beside it. Every PKI view, the
 * inspector and the chase HUD draw states through these, so "revoked" looks the same everywhere.
 */

import { CERT_STATUS, STATUS_ORDER, type CertStatus, type StatusShape } from "../../lib/pkiCert.js";

/** The shape of one state, 12 px, in `currentColor`. */
export function StatusGlyph({ shape, size = 12 }: { readonly shape: StatusShape; readonly size?: number }): React.JSX.Element {
  const common = { width: size, height: size, viewBox: "0 0 12 12", "aria-hidden": true as const, className: "pki-glyph" };
  switch (shape) {
    case "check":
      return (
        <svg {...common}>
          <circle cx="6" cy="6" r="5.5" fill="currentColor" />
          <path d="M3.4 6.2 5.2 8 8.7 4.3" fill="none" stroke="var(--bg-panel)" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      );
    case "clock":
      return (
        <svg {...common}>
          <circle cx="6" cy="6" r="4.9" fill="none" stroke="currentColor" strokeWidth="1.3" />
          <path d="M6 3.3V6l2 1.3" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" />
        </svg>
      );
    case "dash":
      return (
        <svg {...common}>
          <circle cx="6" cy="6" r="4.9" fill="none" stroke="currentColor" strokeWidth="1.3" />
          <path d="M3.6 6h4.8" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
        </svg>
      );
    case "ban":
      return (
        <svg {...common}>
          <circle cx="6" cy="6" r="5.5" fill="currentColor" />
          <path d="M3.3 3.3l5.4 5.4" stroke="var(--bg-panel)" strokeWidth="1.6" strokeLinecap="round" />
        </svg>
      );
    case "shield":
      return (
        <svg {...common}>
          <path d="M6 0.6 10.8 2.4V6c0 2.7-2 4.6-4.8 5.4C3.2 10.6 1.2 8.7 1.2 6V2.4z" fill="currentColor" />
          <path d="M6 3.2v3.3" stroke="var(--bg-panel)" strokeWidth="1.5" strokeLinecap="round" />
          <circle cx="6" cy="8.6" r="0.85" fill="var(--bg-panel)" />
        </svg>
      );
    case "hourglass":
      return (
        <svg {...common}>
          <path d="M3 1h6M3 11h6M3.6 1.2c0 3 4.8 3 4.8 4.8S3.6 7.8 3.6 10.8M8.4 1.2c0 3-4.8 3-4.8 4.8s4.8 1.8 4.8 4.8" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
        </svg>
      );
    case "dots":
    default:
      return (
        <svg {...common}>
          <circle cx="6" cy="6" r="4.9" fill="none" stroke="currentColor" strokeWidth="1.2" strokeDasharray="1.6 1.6" />
        </svg>
      );
  }
}

/** A state as a badge: shape and label, the meaning as its tooltip. */
export function CertBadge({
  status,
  label,
  title,
  testId,
  compact = false,
}: {
  readonly status: CertStatus;
  /** Overrides the state's own label (`Valid · in use`). */
  readonly label?: string;
  readonly title?: string;
  readonly testId?: string;
  /** Only the shape, the label kept for screen readers. */
  readonly compact?: boolean;
}): React.JSX.Element {
  const info = CERT_STATUS[status];
  return (
    <span
      className={`pki-badge st-${status}${compact ? " compact" : ""}`}
      style={{ color: info.color }}
      title={title ?? `${info.label}: ${info.meaning}`}
      data-status={status}
      data-testid={testId}
    >
      <StatusGlyph shape={info.shape} />
      {compact ? <span className="sr-only">{label ?? info.label}</span> : <span className="pki-badge-label">{label ?? info.label}</span>}
    </span>
  );
}

/** The states, in one line, each with its meaning on hover: the key to every certificate colour. */
export function StatusLegend({ only, testId }: { readonly only?: readonly CertStatus[]; readonly testId?: string }): React.JSX.Element {
  return (
    <span className="pki-status-legend" data-testid={testId}>
      {(only ?? STATUS_ORDER).map((s) => (
        <CertBadge key={s} status={s} />
      ))}
    </span>
  );
}
