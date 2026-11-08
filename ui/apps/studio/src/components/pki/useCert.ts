/**
 * One certificate as `inspect.cert` answers it (the decoded certificate and the validation trace a
 * receiver runs), asked once per HashedId8: the certificate card's data.
 */

import { useEffect, useMemo, useState } from "react";

import { certOf, type CertAnswer } from "../../lib/pki.js";
import { certFacts, certStatus, whyNot, type CertStatus } from "../../lib/pkiCert.js";
import { engine } from "../../state/engine.js";

/** One `inspect.cert` answer, asked once per id8. */
export function useCert(id8: string | null): { cert: CertAnswer | null; error: string | null } {
  const [cert, setCert] = useState<CertAnswer | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    setCert(null);
    setError(null);
    if (!id8) return undefined;
    let live = true;
    engine
      .request("inspect.cert", { hashedid8: id8 }, { quiet: true, timeoutMs: 8000 })
      .then((a) => {
        if (!live) return;
        const c = certOf(a);
        setCert(c);
        if (!c) setError("The engine answered, but not with a certificate.");
      })
      .catch((err: unknown) => live && setError(err instanceof Error ? err.message : String(err)));
    return () => {
      live = false;
    };
  }, [id8]);
  return { cert, error };
}

/**
 * A certificate's state in the shared language, from the same `inspect.cert` answer the card reads:
 * what a tile beside the card shows, so the two can never disagree.
 */
export function useCertStatus(id8: string | null, role: string | null = null): { readonly status: CertStatus; readonly why: string | null; readonly zeroLifetime: boolean } {
  const { cert } = useCert(id8);
  return useMemo(() => {
    if (!cert) return { status: "unknown" as CertStatus, why: null, zeroLifetime: false };
    const facts = certFacts(cert.tree, role);
    const v = facts.validity;
    return { status: certStatus(cert, v), why: whyNot(cert), zeroLifetime: v !== null && v.end === v.start };
  }, [cert, role]);
}
