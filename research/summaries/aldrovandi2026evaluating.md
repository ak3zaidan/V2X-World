# Evaluating Post-Quantum TLS Performance for the Internet of Things Using Raspberry Pi Devices
Riccardo Aldrovandi, Stefan Forsström · 2026 IEEE International Workshop on Metrology for Industry 4.0 & IoT (MetroInd4.0 & IoT) 2026 · https://doi.org/10.1109/metroind4.0iot69397.2026.11653116 · relevance: low · basis: abstract

**Problem:** Post-Quantum Cryptography is approaching deployment in TLS, raising important questions for the IoT, where large numbers of long-lived and resource-constrained edge devices must remain secure over decades. Many IoT systems rely on low-cost, Linux-capable edge nodes with limited compute, memory, and power resources, making post-quantum TLS a systems-level concern rather than a ...
**Approach:** This paper benchmarks representative classical and post-quantum TLS primitives on Raspberry Pi 1, 3, 4, used as proxies for constrained IoT edge devices bridging microcontrollers and cloud infrastructure. A PQC-enabled TLS-like handshake introduces modest overhead on modern IoT-class hardware but significantly higher cost on older platforms, highlighting the impact of device heterogeneity in deployed IoT fleets.
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** Results show that lightweight symmetric primitives such as Ascon-128 consistently outperform traditional TLS AEADs, while ML-KEM-512 and hybrid signature configurations provide the most balanced trade-offs among post-quantum public-key schemes.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
