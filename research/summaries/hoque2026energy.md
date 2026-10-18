# Energy-Aware System-Level Evaluation of Post-Quantum TLS on Embedded User Equipment over a Disaggregated 5G Network
Sanzida Hoque, Abdullah Aydeger · 2026 IEEE 51st Conference on Local Computer Networks (LCN) 2026 · https://doi.org/10.1109/lcn67947.2026.11660817 · relevance: low · basis: abstract

**Problem:** The transition to quantum-resistant security is a critical priority for the next generation of mobile networks, particularly within the disaggregated architecture of 5G. This paper presents an energy-aware system-level evaluation of Post- Quantum Cryptography (PQC) integrated into the Transport Layer Security (TLS) handshake on embedded User Equipment (UE).
**Approach:** This paper presents an energy-aware system-level evaluation of Post- Quantum Cryptography (PQC) integrated into the Transport Layer Security (TLS) handshake on embedded User Equipment (UE). Using Raspberry Pi 5s as representative embedded processing platforms, we evaluate the performance of NIST-standardized combinations of classical and post-quantum signature and key exchange mechanisms (KEM), incorporating direct ...
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** Hash-based signature schemes incur up to 4x higher latency and 2x energy compared to lattice-based alternatives, while the impact of KEMs is comparatively smaller.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
