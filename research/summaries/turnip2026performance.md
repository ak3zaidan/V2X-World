# Performance and Scalability of Hybrid Post-Quantum TLS 1.3 on Edge IoT Platforms under Concurrent-Client Load
Togu Turnip, Birger Andersen, Cesar Vargas-Rosales · venue not stated 2026 · https://doi.org/10.2139/ssrn.6991673 · relevance: low · basis: abstract

**Problem:** As Transport Layer Security (TLS) 1.3 migrates to quantum-resistant hybrid post-quantum cryptography (PQC), a growing population of edge Internet of Things (IoT) devices must terminate these heavier handshakes on their own. Yet how hybrid PQC-TLS performs and scales on this intermediate, Linux-class tier of hardware, between the constrained microcontrollers and the capable ...
**Approach:** We close that gap with a controlled performance and scalability study of hybrid PQC-TLS 1.3 on two representative edge platforms, the NVIDIA Jetson Orin Nano and the Raspberry Pi 4B, sweeping NIST security categories L1/L2 through L5 and concurrent-client loads from 5 to 50, with per-stage instrumentation that separates cryptographic computation from protocol-stack and request-handling overhead. Third, hardware ...
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** First, the cryptography is not the bottleneck: lattice operations account for only 2–5% of handshake latency, while the protocol-stack and request-handling cost of moving the enlargedhybrid certificate, key share, and signature payloads dominates. Second, latency scales super-linearly beyond roughly 20 concurrent clients, by 7.5–10.8×, with the 95th-percentile-to-mean ratio widening from 1.4–1.7 to 2.0–2.2, even ...
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
