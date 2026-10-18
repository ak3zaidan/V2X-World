# Signature Placement in Post-Quantum TLS Certificate Hierarchies: An Experimental Study of ML-DSA and SLH-DSA in TLS 1.3 Authentication
J. L. Delgado · IACR Cryptology ePrint Archive 2026 · https://arxiv.org/abs/2604.06100 · relevance: low · basis: abstract

**Problem:** Post-quantum migration in TLS 1.3 couples signature-algorithm choice with certificate-hierarchy structure, chain exposure during the handshake, and role-dependent cryptographic cost. In certificate-based authentication, the practical effect of a signature family depends on where it appears in the certification hierarchy, how much of that hierarchy is exposed during the ...
**Approach:** Post-quantum TLS migration must therefore be evaluated as cryptographic design within authenticated key establishment, with algorithm selection assessed in its deployment context. This paper presents a local experimental study of TLS 1.3 authentication strategies implemented with OpenSSL 3 and oqsprovider.
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** The results also show that transport size alone does not explain the heavy regime: outside leaf-SLH scenarios, transferred bytes and observed chain size track latency closely, but once SLH-DSA reaches the leaf, server-side cryptographic cost becomes dominant.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
