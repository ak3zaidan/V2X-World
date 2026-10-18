# Security and Performance Analyses of Post-Quantum Digital Signature Algorithms and Their TLS and PKI Integrations
Manohar Raavi, Qaiser Khan, Simeon Wuthier, Pranav Chandramouli et al. · Cryptography 2025 · https://doi.org/10.3390/cryptography9020038 · relevance: low · basis: abstract

**Problem:** Quantum computing challenges the mathematical problems anchoring the security of the classical public key algorithms.
**Approach:** Finding common ground to compare these algorithms can be difficult because of their design differences, including the fundamental math problems (lattice-based vs. hash-based). We use a visualization model to show the key/signature size vs. security trade-offs for all PQC algorithms.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** Our results show that the lattice-based algorithms of Dilithium and Falcon induce lower computational overheads than the hash-based algorithms of SPHINCS+. In addition, the lattice-based PQC can outperform the classical algorithm with comparable security strength; for example, Dilithium 2 and Falcon 512 outperform RSA 4096 in the TLS handshake time duration.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — post-quantum/hybrid authentication scheme; the engine implements PQ and hybrid signatures, but the abstract states no SCMS-scale or traffic-coupled evaluation.
**Novelty implications:** We cannot claim PQ authentication for vehicles is new; the end-to-end safety cost of PQ in a full SCMS remains open as far as this abstract shows.
