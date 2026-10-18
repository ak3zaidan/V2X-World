# HySecure: FPGA-Based Hybrid Post-Quantum and Classical Cryptography Platform for End-to-End IoT Security
Bohao Zhang, Jinfa Hong, Gao-Yu Mao, Shi-ying Shen et al. · Electronics 2025 · https://doi.org/10.3390/electronics14193908 · relevance: low · basis: abstract

**Problem:** As the Internet of Things (IoT) continues to expand into mission-critical and long-lived applications, securing low-power wide-area networks (LPWANs) such as Narrowband IoT (NB-IoT) against both classical and quantum threats becomes imperative.
**Approach:** This paper proposes HySecure, an FPGA-based hybrid cryptographic platform that integrates both classical elliptic curve and post-quantum schemes to achieve end-to-end (E2E) security for NB-IoT communication. We design a hybrid key establishment protocol combining ECDH and Kyber through HKDF, and a dual-signature scheme using EdDSA and Dilithium to ensure authenticity and integrity during handshake.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** Our architecture, built upon the lightweight RISC-V PULPino platform, incorporates hardware accelerators for X25519, Kyber, Ed25519, and Dilithium. Cryptographic functions are evaluated on FPGA, achieving a 32.2× to 145.4× speedup.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — post-quantum/hybrid authentication scheme; the engine implements PQ and hybrid signatures, but the abstract states no SCMS-scale or traffic-coupled evaluation.
**Novelty implications:** We cannot claim PQ authentication for vehicles is new; the end-to-end safety cost of PQ in a full SCMS remains open as far as this abstract shows.
