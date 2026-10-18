# Post-Quantum Cryptography Migration in Communication Networks: A Systematic Review of Protocol Integration, Performance Trade-Offs, and Transition Readiness
M. Güllü, Esra Söğüt · Electronics 2026 · https://doi.org/10.3390/electronics15204652 · relevance: medium · basis: abstract

**Problem:** The prospect of a cryptographically relevant quantum computer (CRQC) undermines the public-key primitives that secure virtually every layer of modern communication networks, from subscriber authentication in cellular cores to transport-layer key exchange. Adversaries already following a harvest-now, decrypt-later (HNDL) strategy make this threat actionable today for any traffic whose confidentiality must outlast a ...
**Approach:** This paper reports a PRISMA-2020-compliant systematic literature review of PQC migration in communication networks, structured around five research questions covering protocol and scenario coverage, quantitative performance impact, hybrid cryptography strategies, transition barriers, and open research gaps. We propose a five-level PQC transition-readiness maturity model and identify large-scale field validation, ...
**Tools and scale:** Not stated in the abstract; appears to be analytical/cryptographic or review work.
**Results:** The review finds that transport-layer protocols (TLS 1.3, QUIC, DTLS 1.3) dominate the literature, that lattice-based ML-KEM/ML-DSA offer the most favourable latency–bandwidth–energy trade-off among standardised families, and that classical–PQC hybrid schemes are the de facto transition strategy across 5G/6G, IoT, VPN, and automotive scenarios, at the cost of 10–100× larger keys/signatures and 8–40% handshake ...
**Limitations and future work (as stated by the authors):**
- We propose a five-level PQC transition-readiness maturity model and identify large-scale field validation, PQC–QKD integration, and lightweight signature schemes as priority directions for future work.
**Overlap with our engine:** none — cryptographic/authentication scheme or analysis without the closed-loop simulation, SCMS backend or counterfactual machinery of our engine.
**Novelty implications:** No effect on our novelty claims. 
