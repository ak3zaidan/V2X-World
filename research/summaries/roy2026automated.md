# Automated Post-Quantum Readiness Assessment for Internet-Facing TLS and PKI Infrastructure
Hridoy Roy · venue not stated 2026 · https://doi.org/10.2139/ssrn.7324778 · relevance: low · basis: abstract

**Problem:** Post-quantum migration is often framed as replacing a small set of public-key algorithms, yet operational infrastructures expose a broader systems problem: organizations must determine what cryptography they actually deploy, where it is used, how it relates to certificates and trust chains, what parts of the environment are exposed to harvest-now-decrypt-later (HNDL) threats, ...
**Approach:** This study develops a prototype assessment pipeline that converts externally observable TLS and certificate evidence into a structured cryptographic inventory, an explainable readiness score, and a migration-priority classification. The design is anchored in NIST's finalized FIPS 203-205 standards, NIST's cryptographic-agility and migration guidance, current U.S. inventory and migration requirements, and the 2026 ...
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** Mean local scan time was approximately 28.5 ms per endpoint over ten repetitions per configuration.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
