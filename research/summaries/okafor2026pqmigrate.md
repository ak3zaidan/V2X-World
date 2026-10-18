# PQMigrate: A Backward-Compatible Post-Quantum Migration Blueprint for Wallets and Light Clients
Collins Izuchukwu Okafor, Love Allen Chijioke Ahakonye, Dong-Seong Kim, Jae Min Lee · International Conference on Ubiquitous and Future Networks 2026 · https://doi.org/10.1109/icufn69619.2026.11628510 · relevance: low · basis: abstract

**Problem:** Wallets and light clients are the most exposed cryptographic endpoints in public blockchains, but deployed stacks are still largely ECC-centric. As post-quantum cryptography (PQC) moves into deployment, endpoint migration must remain backward-compatible, preserve recovery and UX flows, and stay efficient on mobile and intermittent links.
**Approach:** We present PQMigrate, a practical migration blueprint that combines (i) hybrid session establishment (ECDHE + ML-KEM) for harvest-now, decryptlater resilience and (ii) transcript-bound dual authorization for high-value actions using classical signatures with ML-DSA/SLH-DSA. The design is chain-agnostic and does not require immediate consensus changes.
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** Results show modest overhead for routine RPC traffic on broadband links, with the dominant cost concentrated in infrequent, critical actions and in constrained-network environments.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
