# Diagnostic Case Study: Client-Side and Ecosystem Barriers in Integrating Post-Quantum Cryptography Kyber768 Into TLS 1.3
Didit Suhartono, K. Isnaini, Azhari Shouni Barkah, Syifalita Hidawani · 2026 International Conference on Current Research in Artificial Intelligence and Data Science (ICCRAIDS) 2026 · https://doi.org/10.1109/iccraids67816.2026.11519730 · relevance: low · basis: abstract

**Problem:** The advancement of quantum computing poses a significant threat to widely deployed public-key cryptographic algorithms. In response, post-quantum cryptography (PQC) integration within Transport Layer Security (TLS) 1.3 is recommended as a transitional strategy.
**Approach:** This paper presents a diagnostic case study analyzing the real-world deployment barriers of integrating the CRYSTALS-Kyber768 key encapsulation mechanism into TLS 1.3. Through detailed diagnostic analysis, this study identifies two primary ecosystem barriers: the lack of client-side capability to advertise Kyber-supported groups in the ClientHello message, and intermediary infrastructure (e.g., CDNs) terminating ...
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** The abstract gives no specific quantitative results.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
