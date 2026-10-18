# Beyond the Handshake: PSKs, Session Resumption, and KeyUpdate in Post-Quantum TLS Migration
Pinky Sarkar · venue not stated 2026 · https://doi.org/10.2139/ssrn.7362158 · relevance: low · basis: abstract

**Problem:** Post-quantum migration of Transport Layer Security (TLS) is frequently framed as the replacement of classical key-exchange mechanisms with post-quantum alternatives. That framing is incomplete because TLS 1.3 derives security through a sequence of secrets and lifecycle mechanisms that extend beyond the initial handshake.
**Approach:** This paper examines how these mechanisms should be interpreted and managed during post-quantum TLS migration. The paper proposes a lifecycle-oriented classification and migration decision framework that treats fresh hybrid key establishment, PSK resumption, 0-RTT, and KeyUpdate as separate security controls.
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** The abstract gives no specific quantitative results.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
