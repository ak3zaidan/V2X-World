# Migration to Quantum Safe Blockchains: A Compact Architecture Using MPPK Key Encapsulation Mechanism and HPPK Digital Signatures
Michel Barbeau, Randy Kuang · Quantum Information Technologies Journal 2026 · https://doi.org/10.13052/qitj2795-0492.217 · relevance: low · basis: abstract

**Problem:** The advent of quantum computing threatens the public-key cryptography that underlies modern blockchains, including ECDSA, Ed25519, and ECDH. Although NIST has standardized lattice-based schemes (ML-KEM and ML-DSA) as quantum-safe standards, their large key and signature sizes (several kilobytes each) pose scalability challenges for blockchains.
**Approach:** We propose a quantum-safe blockchain architecture based entirely on multivariate polynomial cryptography, specifically the MPPK KEM key encapsulation mechanism and HPPK DS digital signature scheme. We present a blockchain design that natively integrates these primitives.
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** Using their linear configuration, MPPK KEM, and HPPK DS produce compact public keys (196-536 bytes) and signatures (144-272 bytes) – considerably smaller than ML-DSA’s 2–3 KB.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
