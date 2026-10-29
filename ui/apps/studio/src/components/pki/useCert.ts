/**
 * One certificate as `inspect.cert` answers it (the decoded certificate and the validation trace a
 * receiver runs), asked once per HashedId8: the certificate card's data.
 */

import { useEffect, useState } from "react";

import { certOf, type CertAnswer } from "../../lib/pki.js";
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
