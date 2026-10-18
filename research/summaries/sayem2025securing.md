# Securing TLS in the Quantum Era: A Robust Hybrid Protocol with Kyber, X25519, and Dilithium
Abu Sayem, Tahmid Enam Shrestha, Farzana Rahman, Md Asif Khan et al. · 2025 28th International Conference on Computer and Information Technology (ICCIT) 2025 · https://doi.org/10.1109/iccit68739.2025.11491661 · relevance: low · basis: abstract

**Problem:** The rise of quantum computers threatens the security of current TLS protocols, which rely on elliptic curve and RSA-based primitives that quantum algorithms like Shor's can break. This creates a “store now, decrypt later” risk, where attackers may collect encrypted data today to decrypt it in the future once quantum capabilities advance.
**Approach:** This study introduces a novel hybrid TLS protocol that seamlessly integrates classical (ECDSA and X25519) and post-quantum (CRYSTALS-Dilithium5 and CRYSTALS-Kyber1024) primitives. The proposed method generates a composite X. 509 certificate dual-signed with ECDSA and Dilithium5.
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** Experimental validation via a proof-of-concept demonstrates successful hybrid certificate validation and secure session establishment with both classical and post-quantum peers. These results confirm that hybrid cryptographic integration can provide quantum-resilient security while preserving compatibility with existing TLS deployments.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
