# Internet X.509 Public Key Infrastructure Permanent Identifier
D. Pinkas, T. Gindin · Request for Comments 2005 · https://doi.org/10.17487/rfc4043 · relevance: low · basis: abstract

**Problem:** This document defines a new form of name, called permanent identifier, that may be included in the subjectAltName extension of a public key certificate issued to an entity.
**Approach:** The permanent identifier is an optional feature that may be used by a CA to indicate that two or more certificates relate to the same entity, even if they contain different subject name (DNs) or different names in the subjectAltName extension, or if the name or the affiliation of that entity stored in the subject or another name form in the subjectAltName extension has changed.
**Tools and scale:** Not stated in the abstract.
**Results:** No quantitative results stated in the abstract.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — low-relevance adjacent work (general PKI/revocation background); no SCMS/1609.2 PKI backend, closed-loop revocation, joint ground truth or counterfactual reruns indicated.
**Novelty implications:** Does not rule out any candidate direction; background only.
