# A Readiness and Maturity Framework for Post-Quantum TLS Adoption
Francis Kagai · venue not stated 2026 · https://doi.org/10.20944/preprints202606.0067.v1 · relevance: low · basis: abstract

**Problem:** Transport Layer Security (TLS) protects most Internet traffic, yet migration to post-quantum TLS (PQ-TLS) depends on more than publishing a new algorithm standard. Despite rapid standardization of the Module-Lattice-Based Key-Encapsulation Mechanism (ML-KEM) and hybrid TLS groups, practitioners lack a structured way to assess whether deployment ecosystems are ready to adopt ...
**Approach:** This letter proposes a six-dimensional readiness and adoption framework and a reproducible maturity rubric (levels 0--4).
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** The main finding is that cryptographic and protocol readiness are uniformly high across the case study, while platform and application readiness show the widest gap and act as the primary adoption bottlenecks for software that inherits OS TLS APIs.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
