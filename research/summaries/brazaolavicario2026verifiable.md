# Verifiable post-quantum authentication for QKD relays without public key infrastructure
Aitor Brazaola-Vicario, Oscar Lage, Eduardo Jacob, Jasone Astorga · Journal of Optical Communications and Networking 2026 · https://doi.org/10.1364/jocn.589831 · relevance: low · basis: abstract

**Problem:** We propose a lightweight post-quantum authentication scheme for multi-hop QKD relay networks, designed to replace client-side mutual transport layer security (TLS) with a more efficient and verifiable mechanism. The protocol combines an additional random source, a Wegman–Carter message authentication code (MAC), and a Dilithium-2 signature, providing 128-bit integrity and ...
**Approach:** The protocol combines an additional random source, a Wegman–Carter message authentication code (MAC), and a Dilithium-2 signature, providing 128-bit integrity and strong post-quantum guarantees without relying on client certificates or traditional public key infrastructures. This design eliminates the latency and complexity associated with repeated TLS handshakes, while enabling path-level auditing through chained ...
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** Experimental results show a 28% reduction in end-to-end latency compared to mutual TLS, stable signature costs below 0.4 ms, and predictable payload growth under chained verification, confirming both practicality and scalability.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
