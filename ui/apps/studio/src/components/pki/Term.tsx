/**
 * A term or acronym with its meaning one hover or one Tab away: dotted underline, the glossary's
 * sentence in a small popover on hover and on keyboard focus, and the same sentence as the
 * accessible description. Every certificate view uses this one component, so no acronym is left
 * unexplained and none is explained twice in different words (`lib/pkiGlossary.ts`).
 */

import { glossary } from "../../lib/pkiGlossary.js";

export function Term({ k, children }: { readonly k: string; readonly children?: React.ReactNode }): React.JSX.Element {
  const g = glossary(k);
  if (!g) return <>{children ?? k}</>;
  return (
    <span role="term" className="pki-term" tabIndex={0} data-def={`${g.term}: ${g.means}`} aria-description={g.means} data-testid="pki-term" data-term={k}>
      {children ?? g.term}
    </span>
  );
}
