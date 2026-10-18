# A Hybrid Quantum-Resistant Resumption framework for zero-RTT TLS 1.3
Tahera Begum Abdul, Venkata Ramana Kondapalli · Frontiers of Computer Science 2026 · https://doi.org/10.3389/fcomp.2026.1848687 · relevance: low · basis: abstract

**Problem:** Zero-round-trip-time (0-RTT) resumption is a central performance feature of TLS 1.3, cutting reconnection latency by letting clients attach early application data to pre-shared keys derived from previously issued session tickets. Latency-sensitive deployments—web services, API gateways, IoT fleets-rely heavily on this mechanism.
**Approach:** Latency-sensitive deployments—web services, API gateways, IoT fleets-rely heavily on this mechanism. This paper introduces HQRT (Hybrid Quantum-Resistant Resumption for TLS 1.3), which folds a hybrid X25519 + ML-KEM-768 encapsulation directly into the NewSessionTicket lifecycle, rendering tickets quantum-safe without adding any extra round trip.
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** Zero-round-trip-time (0-RTT) resumption is a central performance feature of TLS 1.3, cutting reconnection latency by letting clients attach early application data to pre-shared keys derived from previously issued session tickets. The latency penalty introduced by HQRT is just 4–9%, with a throughput loss of 6.5% relative to classical 0-RTT-a stark contrast to the 81–89% overhead imposed by full post-quantum ...
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
