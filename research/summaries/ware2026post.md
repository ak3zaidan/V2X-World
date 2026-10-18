# Post-Quantum Migration for Secure Protocols: Building a Quantum-Safe Internet
Sachin Ware, S. Wankhade, C. Sukte, Manohar K. Kodmelwar et al. · 2026 International Conference on Advancements in Smart, Secure and Intelligent Computing (ASSIC) 2026 · https://doi.org/10.1109/assic70037.2026.11717285 · relevance: low · basis: abstract

**Problem:** The short boom of quantum computing is a critical threat to traditional cryptographic requirements like RSA & ECC. this means we need to replace to post-Quantum Cryptography proper away. This research presents an elaborate, enterprise-level migration process for PQC based on a hybrid TLS model that provides backwards compatibility and utilizes quantum-resistant algorithms.
**Approach:** This research presents an elaborate, enterprise-level migration process for PQC based on a hybrid TLS model that provides backwards compatibility and utilizes quantum-resistant algorithms. To verify the applicability of such a model in practice, experiments were carried out on an Intel Core i7-powered platform using the OQS library.
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** The hybrid handshake model, which combines X25519 with ML-KEM-768, only adds a latency overhead of 1.2 to 1.5 milliseconds and increases the CPU usage from 3% to 7%.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
