# A Practical Performance Benchmark of Post-Quantum Cryptography Across Heterogeneous Computing Environments
Maryam Abbasi, Filipe Cardoso, Paulo Váz, J. Silva et al. · Cryptogr. 2025 · https://doi.org/10.3390/cryptography9020032 · relevance: medium · basis: abstract

**Problem:** The emergence of large-scale quantum computing presents an imminent threat to contemporary public-key cryptosystems, with quantum algorithms such as Shor’s algorithm capable of efficiently breaking RSA and elliptic curve cryptography (ECC). This vulnerability has catalyzed accelerated standardization efforts for post-quantum cryptography (PQC) by the U.S.
**Approach:** This paper presents the most extensive cross-platform empirical evaluation to date of NIST-selected PQC algorithms, including CRYSTALS-Kyber and NTRU for key encapsulation mechanisms (KEMs), alongside BIKE as a code-based alternative, and CRYSTALS-Dilithium and Falcon for digital signatures.
**Tools and scale:** While theoretical security analysis of these quantum-resistant algorithms has advanced considerably, comprehensive real-world performance benchmarks spanning diverse computing environments—from high-performance cloud infrastructure to severely resource-constrained IoT devices—remain insufficient ...
**Results:** To address this, we propose and evaluate three optimization strategies that reduce bandwidth requirements by 40–60% without compromising security guarantees. Our results conclusively demonstrate that PQC is deployment-ready for most applications, provided that implementations are carefully optimized for the specific performance characteristics and security requirements of target environments.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — PQC performance benchmark across platforms.
**Novelty implications:** PQ cost numbers available as inputs.
