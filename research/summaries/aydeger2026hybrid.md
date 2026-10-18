# When Hybrid TLS Is Not Enough: Parser Robustness, API Exposure, and Policy Drift in Post-Quantum TLS 1.3
Abdullah Aydeger, Engin Zeydan · 2026 IEEE Conference on Communications and Network Security (CNS) 2026 · https://doi.org/10.1109/cns69863.2026.11720907 · relevance: low · basis: abstract

**Problem:** Hybrid post-quantum (PQ) key exchange is deployed at scale in TLS 1.3, typically combining X25519 with ML-KEM-768 in a single key_share. TLS 1.3’s transcript authentication forecloses ordinary on-path negotiation tampering, and prior work has measured the performance cost of PQ cryptography.
**Approach:** We argue that hybrid PQ TLS may still be insufficient in practice, not for cryptographic reasons but at the deployment envelope: parser edge cases for hybrid key_share payloads, draft-era identifier coexistence, opaque APIs that prevent applications from enforcing “hybrid or fail,” permissive defaults, and TLS-terminating middleboxes that legitimately negotiate classical traffic in front of PQ-capable backends. We ...
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** The abstract gives no specific quantitative results.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
