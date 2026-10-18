# Integration of Quantum-Safe Algorithms into X.509v3 Certificates
Congli Wang, Weijia Xue, Jingran Wang · 2023 IEEE 3rd International Conference on Electronic Technology, Communication and Information (ICETCI) 2023 · https://doi.org/10.1109/icetci57876.2023.10176713 · relevance: low · basis: abstract

**Problem:** Algorithms such as RSA and ECC, based on the mathematical problems of integer factorization or discrete logarithm, are commonly used However, these problems can be solved in polynomial time on a large-scale quantum computer with Shor’s algorithm.
**Approach:** To counter this threat, the cryptography community is actively developing post-quantum cryptography (PQC). To enable public key cryptography algorithms, including PQC algorithms, it is necessary to solve a problem of associating the cryptographic key with the entity it belongs to, usually done through public key infrastructure (PKI) and digital certifica In this paper, we explore how to integrate quantum-safe algorithms into X.509v3 certificates, analyze the schemes for migrating to quantum-safe certificates, compare them, and provide migration recommendations.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** No quantitative results stated in the abstract.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — post-quantum/hybrid authentication scheme; the engine implements PQ and hybrid signatures, but the abstract states no SCMS-scale or traffic-coupled evaluation.
**Novelty implications:** We cannot claim PQ authentication for vehicles is new; the end-to-end safety cost of PQ in a full SCMS remains open as far as this abstract shows.
