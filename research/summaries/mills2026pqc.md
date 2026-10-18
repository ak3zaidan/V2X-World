# PQC-OAV: A Unified Evidential Assurance Framework for Post-Quantum Transport (TLS, SSH, QUIC)
Michael MILLS, Ali Ghorbani · venue not stated 2026 · https://doi.org/10.2139/ssrn.6804401 · relevance: low · basis: abstract

**Problem:** The transition to Post-Quantum Cryptography introduces critical observability challenges. Large hybrid keys and encapsulated ciphertexts systematically disrupt traditional network monitoring tools.
**Approach:** This paper generalizes the PQC-OAV architecture to establish a strictly protocol-agnostic, passive assurance framework. We define a formal Unified Tuple (Ts), a normalized feature vector that maps disparate protocol identifiers into a canonical format.
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** We demonstrate that while TCP stacks suffer severe latency compounding under post-quantum loads, UDP multiplexing maintains robust resilience on standard links but undergoes complete structural collapse under constrained MTUs, accounting for 300 socket-layer rejections.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
