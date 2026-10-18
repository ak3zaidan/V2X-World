# PureFL: Trust-Weighted Federated Learning With Noise-Resilient Homomorphic Encryption for Blockchain-Based IoV Networks
H. L. Nakayiza, Love Allen Chijioke Ahakonye, Dong-Seon Kim, Jae Min Lee · IEEE Internet of Things Journal 2026 · https://doi.org/10.1109/jiot.2026.3691349 · relevance: medium · basis: abstract

**Problem:** Federated learning (FL) in the Internet of Vehicles (IoV) networks is challenged by privacy vulnerabilities, model poisoning attacks, and the limitations of centralized aggregation, which threaten security and scalability.
**Approach:** While homomorphic encryption (HE) provides an additional layer of privacy, existing schemes often suffer from cumulative noise growth, resulting in degraded model accuracy and increased inference latency, critical factors for real-time vehicular safety applications. This article presents PureFL, a novel FL framework that combines adaptive Cheon-Kim-Kim-Song (CKKS) HE with a custom permissioned blockchain (PureChain), utilizing a proof of authority and association (PoA2) consensus mechanism.
**Tools and scale:** Mentioned in abstract: 5G, blockchain
**Results:** Experimentation results on the CICIOV2024 and 5G-NIDD datasets demonstrate that PureFL achieves superior anomaly detection accuracy of 99.9%, reduces CKKS HE overhead via precision scaling, and the proposed PoA2 consensus outperforms other consensus mechanisms in terms of throughput and latency, making it suitable for real-time IoV operations.
**Limitations and future work (as stated by the authors):**
- Federated learning (FL) in the Internet of Vehicles (IoV) networks is challenged by privacy vulnerabilities, model poisoning attacks, and the limitations of centralized aggregation, which threaten security and scalability.
**Overlap with our engine:** partial — same area (V2X/vehicular security or networking) with a narrower, mechanism-level focus.
**Novelty implications:** Does not remove any of our claims beyond its specific mechanism; we cannot claim to be first on that mechanism. The joint closed-loop traffic/radio/PKI evaluation with counterfactual ground truth remains open as far as this abstract shows.
