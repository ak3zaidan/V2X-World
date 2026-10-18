# Post-Quantum Public Key Infrastructures: Hybrid Certificates, Cryptographic Combiners, and Migration Strategies
Abdullah Aydeger, Engin Zeydan, Awaneesh Kumar Yadav, Madhusanka Liyanage · 2026 IEEE 23rd Consumer Communications &amp;amp; Networking Conference (CCNC) 2026 · https://doi.org/10.1109/ccnc65079.2026.11366420 · relevance: low · basis: abstract

**Problem:** This paper addresses the multifaceted challenges of integrating PQC into existing PKI ecosystems by examining novel cryptographic combiners and hybrid certificate designs that can provide quantum-resistant security.
**Approach:** We assess the performance, compatibility, and security of these hybrid certificates across standard communication protocols such as TLS, considering variations in root and intermediate certification paths. In addition, we introduce key management procedures that cover signature generation, validation, and lifecycle considerations under both software and hardware constraints.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** Beyond X.509, alternative trust models and certificate mechanisms are also analyzed for specialized domains, including IoT, firmware signing, and smart cards.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — post-quantum/hybrid authentication scheme; the engine implements PQ and hybrid signatures, but the abstract states no SCMS-scale or traffic-coupled evaluation.
**Novelty implications:** We cannot claim PQ authentication for vehicles is new; the end-to-end safety cost of PQ in a full SCMS remains open as far as this abstract shows.
