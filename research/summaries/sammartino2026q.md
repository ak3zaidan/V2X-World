# Q-FE: A Quantum-Native 6G Far-Edge Architecture Securing Industrial IoT Digital Twins via CSIDH-PQC and Asynchronous Federated Learning
Vincenzo Sammartino · arXiv.org 2026 · https://arxiv.org/abs/2606.03611 · relevance: medium · basis: abstract

**Problem:** Sixth-generation (6G) wireless networks will underpin ultra-dense Industrial IoT (IIoT) ecosystems in which resource-constrained Far-Edge devices -- autonomous mobile robots, industrial actuators, connected vehicles -- must simultaneously satisfy sub-millisecond latency, $10^{-7}$-class reliability, and decades-long cryptographic security. Current architectures delegate Digital Twin (DT) computation to centralised ...
**Approach:** We propose Q-FE, a Quantum-Native 6G Far-Edge architecture integrating three co-designed components: (i) Micro-Digital Twins ($\mu$DTs) co-located with 6G base stations and high-capability endpoints; (ii) a Cross-Layer Post-Quantum Key Exchange module embedding CSIDH-512 isogeny key material directly within MAC-layer control frames, exploiting the scheme's uniquely compact keys ($\le 64$ bytes) to avoid packet ...
**Tools and scale:** End-to-end simulations (NS-3 + PySyft) demonstrate that Q-FE reduces MAC-layer overhead by 62% versus ML-KEM/Kyber-1024, maintains P99.9 URLLC latency at 0.78 ms, and accelerates global-model convergence by 31% over synchronous Federated Learning.
**Results:** End-to-end simulations (NS-3 + PySyft) demonstrate that Q-FE reduces MAC-layer overhead by 62% versus ML-KEM/Kyber-1024, maintains P99.9 URLLC latency at 0.78 ms, and accelerates global-model convergence by 31% over synchronous Federated Learning.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — cryptographic/authentication scheme or analysis without the closed-loop simulation, SCMS backend or counterfactual machinery of our engine.
**Novelty implications:** No effect on our novelty claims. 
