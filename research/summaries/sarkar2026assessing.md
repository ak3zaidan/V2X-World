# Assessing and Managing Post-Quantum Migration Readiness Across TLS and PKI Infrastructure
Pinky Sarkar · venue not stated 2026 · https://doi.org/10.2139/ssrn.7362119 · relevance: low · basis: abstract

**Problem:** Post-quantum migration is an infrastructure transition rather than a simple substitution of cryptographic algorithms. Public-key mechanisms used in TLS, certificates, certificate authorities, digital signatures, key-management systems, and trust stores create dependencies that can make migration difficult to sequence and validate.
**Approach:** This paper develops a literature-based readiness and migration framework that connects cryptographic discovery, dependency mapping, Harvest Now, Decrypt Later (HNDL) exposure, TLS and PKI assessment, migration prioritization, interoperability validation, and cryptographic agility. The paper therefore distinguishes established standards and guidance from research claims and from the framework proposed here.
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** The resulting model treats readiness as a lifecycle capability: organizations must be able to discover cryptography, understand dependencies, prioritize long-lived and high-value assets, test transition mechanisms, deploy replacements, retire vulnerable mechanisms, and retain the ability to change algorithms again.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
