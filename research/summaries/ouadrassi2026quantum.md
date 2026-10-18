# Toward Quantum-Safe Permissioned Blockchains: Evaluating Hybrid ML-DSA-65/ECDSA Signatures on Hyperledger Fabric
Nihad Lechhab Ouadrassi, Mohammed Said Boukhryss, Abdellah Azmani, Khalid Jebari Hassani · IEEE Access 2026 · https://doi.org/10.1109/access.2026.3735938 · relevance: low · basis: abstract

**Problem:** Post-quantum migration is a critical challenge for enterprise blockchain systems that rely on classical digital signatures. Hyperledger Fabric extensively uses ECDSA for identity management, endorsement, TLS, and transaction validation, making a full infrastructure-level migration complex.
**Approach:** This paper presents a controlled empirical evaluation of a hybrid ML-DSA-65/ECDSA signature architecture for Hyperledger Fabric. The proposed approach preserves Fabric’s native ECDSA-based infrastructure while integrating ML-DSA-65 at the application layer through an external post-quantum signing microservice.
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** Results show that optimized ML-DSA-65 signing achieves an average latency of 0.226 ms, while no statistically significant degradation was detected in the primary transaction-level performance metrics under the tested workloads. The main measured operational cost is storage-related: active blocks in the ML-DSA-65 configuration are approximately $1.43\times $ larger than those of the functionally equivalent ...
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
