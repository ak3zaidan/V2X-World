# Quantum Shield: Design, Implementation, and Empirical Evaluation of a Hybrid Post-Quantum Secure Peer-to-Peer Communication System Using ML-KEM and ML-DSA
Sridhar S, Balraj S, Anbarasu J, Sanjay M et al. · Indian Journal of Computer Science and Technology 2026 · https://doi.org/10.59256/indjcst.20260502025 · relevance: low · basis: abstract

**Problem:** The emergence of fault-tolerant quantum computing poses an existential threat to classical public-key cryptographic systems, including RSA and Elliptic Curve Cryptography (ECC), through Shor's polynomial-time factorization algorithm. The Harvest Now, Decrypt Later (HNDL) attack paradigm further amplifies this threat by enabling adversaries to archive ciphertext today for ...
**Approach:** This paper presents QuantumShield, a hybrid post-quantum secure peer-to-peer communication platform that integrates ML-KEM-768 for quantum-safe key encapsulation, ML-DSA-65 for authenticated digital signatures, and AES-256-GCM for high-throughput symmetric encryption.
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** Our primary research contribution is a rigorous empirical analysis of PQC integration overhead in a real-world communication pipeline, quantifying latency, key size, and security trade-offs against classical RSA-2048 and ECC-256 baselines under controlled experimental conditions. Experimental evaluation across 1,000 iterations demonstrates that ML-KEM-768 encapsulation completes in 2.8 ± 0.2 ms compared to 4.1 ± 0.3 ...
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
