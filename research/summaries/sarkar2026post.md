# Post-Quantum Certificate Lifecycle Management at Scale: Discovery, Rotation, Revocation, and Trust-Anchor Migration
Pinky Sarkar · venue not stated 2026 · https://doi.org/10.2139/ssrn.7362218 · relevance: low · basis: abstract

**Problem:** The transition to post‑quantum cryptography (PQC) is often described as an algorithm‑replacement problem, but enterprise deployment reveals a broader operational challenge: the lifecycle management of certificates and their associated trust relationships. Public Key Infrastructure (PKI) migration affects not only end‑entity certificates but also intermediate and root ...
**Approach:** This paper examines post‑quantum certificate lifecycle management as a distinct systems‑management problem. It develops a lifecycle‑oriented framework covering cryptographic discovery, certificate inventory, dependency mapping, algorithm classification, issuance, deployment, rotation, revocation, trust‑anchor migration, validation, monitoring, and retirement.
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** The framework is conceptual and literature‑based; it does not claim new production measurements or experimental interoperability results.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
