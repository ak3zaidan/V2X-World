/**
 * A sentence from the engine (what an authority knows, what a message teaches) with every SCMS
 * acronym in it routed through the glossary's `Term`: one hover or one Tab explains it, and the
 * words themselves are left as they are.
 */

import { Fragment } from "react";

import { glossTerms } from "../../lib/pkiGlossary.js";
import { Term } from "./Term.js";

export function GlossText({ text }: { readonly text: string }): React.JSX.Element {
  return (
    <>
      {glossTerms(text).map((p, k) => (p.term ? <Term key={k} k={p.term}>{p.text}</Term> : <Fragment key={k}>{p.text}</Fragment>))}
    </>
  );
}
