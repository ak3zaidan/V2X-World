# Practical Post-Quantum Cryptography for Bandwidth Constrained or Non-Terrestrial Networks, and Power Constrained Devices
Elliot Eichen, Sylvia Llosa, Yue-Qi Chen, Sangtae Ha · arXiv.org 2026 · https://arxiv.org/abs/2607.23007 · relevance: medium · basis: abstract

**Problem:** Post-quantum (PQ) cryptographic algorithms, particularly for authentication, are more complex than classical algorithms and require larger certificates, signatures, and keys. Establishing a PQ-secure network connection increases bandwidth, memory, computation time, and energy consumption.
**Approach:** This paper investigates an alternative cryptographic framework that replaces PQ digital certificates with shared secret keys (SSKs).
**Tools and scale:** Not stated in the abstract; appears to be analytical/cryptographic or review work.
**Results:** Consequently, applications such as key rotation and key management may be unable to achieve acceptable handshake reliability or support NIST PQ Security Categories above Category 1. Compared with certificate-based PQ authentication (e.g., ML-DSA), the proposed approach reduces handshake bandwidth, endpoint RAM, computation time, and energy use while preserving PQ-secure AEAD, including forward secrecy and replay ...
**Limitations and future work (as stated by the authors):**
- Similar limitations affect low-power IoT devices and bandwidth- or energy-constrained terrestrial networks, where PQ authentication may restrict devices to Category 1 security or prevent ambient-powered endpoints from supporting PQ authentication altogether.
**Overlap with our engine:** none — cryptographic/authentication scheme or analysis without the closed-loop simulation, SCMS backend or counterfactual machinery of our engine.
**Novelty implications:** No effect on our novelty claims. 
