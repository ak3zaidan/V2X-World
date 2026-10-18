# Integrating Post-Quantum Cryptography Into a CDN-Style PKI: Design, Trade-Offs, and an Experimental Baseline
Katrina Parsom, Yoohwan Kim, Ju-Yeon Jo · 2026 Silicon Valley Cybersecurity Conference (SVCC) 2026 · https://doi.org/10.1109/svcc69905.2026.11642336 · relevance: low · basis: abstract

**Problem:** Post-quantum cryptography (PQC) adds larger certificates and extra workload to the Transport Layer Security (TLS) handshake. In order to analyze the effect of that change on content delivery, a controlled testbed with conditions replicating a Content Delivery Network (CDN) is used.
**Approach:** The test harness also supports mixed web workloads and records key metrics, including handshake time, page and object latency, throughput, and server CPU utilization. Experiments consist of hot and cold cache scenarios as well as new and keep-alive connections.
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** The abstract gives no specific quantitative results.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
