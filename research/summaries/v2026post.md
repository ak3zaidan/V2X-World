# Post-Quantum Cryptography Migration for Enterprise Security
Mini T. V. · International Journal of Information Technology Research Studies (IJITRS) 2026 · https://doi.org/10.63090/ijitrs/3139.3209.0032 · relevance: low · basis: abstract

**Problem:** Large-scale quantum computers threaten the public-key cryptography that protects enterprise data, communications, and digital identity. Shor's algorithm solves integer factorization and discrete logarithms in polynomial time, which would break RSA, Diffie-Hellman, and elliptic-curve schemes once a cryptographically relevant quantum computer exists.
**Approach:** This paper presents a practical migration framework for enterprises. It reviews the quantum threat and Mosca's inequality for timing the transition, summarizes the new standards with their key and signature sizes, and proposes a five-phase roadmap built on crypto-agility: discovery and inventory, risk prioritization, agility engineering, hybrid deployment, and validation.
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** ML-KEM-768 carries a 1,184-byte public key against 64 bytes for an elliptic-curve key, while SPHINCS+ signatures can exceed 17 kilobytes.
**Limitations and future work (as stated by the authors):**
- "It consolidates published parameters and field guidance into an actionable plan, and it highlights open challenges in certificate sizing, hardware support, and long-lived embedded systems."
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
