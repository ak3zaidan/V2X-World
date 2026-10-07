/**
 * The handful of icons the header uses, drawn inline so they take the text colour in both themes
 * and cost no request. Each is decorative: the button around it carries the accessible name.
 */

const common = {
  width: 16,
  height: 16,
  viewBox: "0 0 16 16",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.4,
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
  "aria-hidden": true,
  focusable: false,
};

export function GearIcon(): React.JSX.Element {
  return (
    <svg {...common}>
      <circle cx="8" cy="8" r="2.2" />
      <path d="M8 1.5v1.7M8 12.8v1.7M1.5 8h1.7M12.8 8h1.7M3.4 3.4l1.2 1.2M11.4 11.4l1.2 1.2M3.4 12.6l1.2-1.2M11.4 4.6l1.2-1.2" />
    </svg>
  );
}

/** Three authorities linked to a device: the Backend view's door. */
export function NetworkIcon(): React.JSX.Element {
  return (
    <svg {...common}>
      <rect x="1.5" y="1.8" width="4" height="3.2" rx="0.6" />
      <rect x="10.5" y="1.8" width="4" height="3.2" rx="0.6" />
      <rect x="6" y="11" width="4" height="3.2" rx="0.6" />
      <path d="M3.5 5v2.5h9V5M8 7.5V11" />
    </svg>
  );
}

export function ChartIcon(): React.JSX.Element {
  return (
    <svg {...common}>
      <path d="M2 13.5h12" />
      <path d="M3.5 11l3-4 2.5 2 3.5-5" />
    </svg>
  );
}

export function InspectorIcon(): React.JSX.Element {
  return (
    <svg {...common}>
      <rect x="1.8" y="2.5" width="12.4" height="11" rx="1.5" />
      <path d="M10 2.5v11" />
    </svg>
  );
}

export function MoreIcon(): React.JSX.Element {
  return (
    <svg {...common} strokeWidth={2.2}>
      <path d="M3.5 8h.01M8 8h.01M12.5 8h.01" />
    </svg>
  );
}

export function CloseIcon(): React.JSX.Element {
  return (
    <svg {...common}>
      <path d="M4 4l8 8M12 4l-8 8" />
    </svg>
  );
}

/** A speech bubble with a spark: the agent's door. */
export function AgentIcon(): React.JSX.Element {
  return (
    <svg {...common}>
      <path d="M2.5 3.5h11v7h-6l-3 2.5v-2.5h-2z" />
      <path d="M8 5.2v2.6M6.7 6.5h2.6" />
    </svg>
  );
}
