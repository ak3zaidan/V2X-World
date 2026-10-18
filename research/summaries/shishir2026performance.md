# Performance and Security Analysis of Post-Quantum Signature Algorithms for Blockchain Systems
Md Shishir · venue not stated 2026 · https://doi.org/10.2139/ssrn.7122722 · relevance: low · basis: abstract

**Problem:** The rise of large-scale quantum computing threatens the cryptographic foundations of blockchain networks. Current platforms depend on the Elliptic Curve Digital Signature Algorithm (ECDSA), which Shor's algorithm can break in polynomial time.
**Approach:** This paper presents a statistically rigorous performance and security comparison of three NIST-standardized postquantum signature schemes-Dilithium2 (ML-DSA, FIPS 204), Falcon-512 (FN-DSA), and SPHINCS+ (SLH-DSA, FIPS 205)benchmarked against ECDSA in a blockchain context. Using the liboqs C library (version 0.11.0), we executed 1 000 benchmark iterations per algorithm and measured key-generation, signing, and ...
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** Dilithium2 achieves the fastest signing (0.093 ms) and verification (0.030 ms) of all evaluated schemes. Falcon-512 offers the most compact post-quantum signatures (655 bytes) with verification at 0.058 ms.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
