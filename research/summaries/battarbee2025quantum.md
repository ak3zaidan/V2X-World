# Quantum-safe hybrid key exchanges with KEM-based authentication
Christopher Battarbee, Christoph Striecks, Ludovic Perret, Sebastian Ramacher et al. · EPJ Quantum Technology 2025 · https://doi.org/10.1140/epjqt/s40507-025-00425-3 · relevance: low · basis: abstract

**Problem:** However, the efficiency of available instantiations of such signature schemes is not yet comparable to that of their post-quantum key-encapsulation mechanism (KEM) counterparts, particularly in large networks with potentially several connections in a short period of time.
**Approach:** In PQCrypto 2023, Bruckner, Ramacher, and Striecks proposed a novel hybrid AKE (HAKE) protocol dubbed Muckle+, which is particularly useful in large quantum-safe networks consisting of a large number of nodes. In order to address this discrepancy, the present work proposes Muckle#, a protocol that aims to expand the existing boundaries of efficiency within the HAKE framework.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** Muckle# utilizes post-quantum KEMs for implicit authentication, drawing inspiration from recent advancements in the domain of Transport Layer Security (TLS) protocols, particularly in KEMTLS (CCS’20). Moreover, we implemented a proof of concept, thereby demonstrating practicality of this alternative approach to authentication within HAKE.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — post-quantum/hybrid authentication scheme; the engine implements PQ and hybrid signatures, but the abstract states no SCMS-scale or traffic-coupled evaluation.
**Novelty implications:** We cannot claim PQ authentication for vehicles is new; the end-to-end safety cost of PQ in a full SCMS remains open as far as this abstract shows.
