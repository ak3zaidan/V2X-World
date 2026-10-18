# Hybrid Post-Quantum IKEv2 on Embedded Automotive Platforms: Design, Implementation, and Evaluation
Ahmed Ayman Bahaa-Eldin, Mohamed Watheq El-Kharashi, Bassem Abdullah · Electronics 2026 · https://doi.org/10.3390/electronics15153340 · relevance: high · basis: abstract

**Problem:** IKEv2/IPsec relies on ECDH, which is quantum-vulnerable; PQ payloads may exceed non-fragmenting budgets.
**Approach:** Hybrid ECDH P-384 + ML-KEM-768 IKEv2 (RFC 9370/9242) with a Fragmentation Boundary Model, implemented on a TI TM4C1294 microcontroller (FreeRTOS, CycloneTCP). (summarised from the abstract only)
**Tools and scale:** Embedded hardware; 150 handshakes per configuration.
**Results:** Mean SA establishment rose from 3390.4 to 3488.6 ms (98.2 ms overhead).
**Limitations and future work (as stated by the authors):**
- Not stated in the available abstract.
**Overlap with our engine:** partial — PQ hybrid key exchange on automotive embedded hardware and fragmentation limits; we model PQ in 1609.2/TLS provisioning.
**Novelty implications:** Embedded hybrid PQ timing exists for IKEv2; not for SCMS provisioning flows.
