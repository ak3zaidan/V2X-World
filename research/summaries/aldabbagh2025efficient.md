# Efficient Post-Quantum Cryptography Algorithms for Auto-Enrollment in Public Key Infrastructure
Rehab Al-Dabbagh, Mohammad Alkhatib, Tahani Albalawi · Electronics 2025 · https://doi.org/10.3390/electronics14101980 · relevance: low · basis: abstract

**Problem:** The security of the digital certificates used in authenticating network devices relies on cryptographic algorithms like the RSA and ECC, which are vulnerable to quantum attacks.
**Approach:** This study addresses the urgent need to secure the Simple Certificate Enrollment Protocol (SCEP), widely used in PKI-based systems, by integrating post-quantum cryptographic (PQC) algorithms—Dilithium, Falcon, and SPHINCS+.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** The experimental results show that Dilithium2 (1312 bytes) and Falcon512 (897 bytes) offer the best performance and throughput, with Falcon512 also being the most efficient in terms of the storage consumption.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — post-quantum/hybrid authentication scheme; the engine implements PQ and hybrid signatures, but the abstract states no SCMS-scale or traffic-coupled evaluation.
**Novelty implications:** We cannot claim PQ authentication for vehicles is new; the end-to-end safety cost of PQ in a full SCMS remains open as far as this abstract shows.
