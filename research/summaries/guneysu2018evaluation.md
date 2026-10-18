# Evaluation of Lattice-Based Signature Schemes in Embedded Systems
T. Güneysu, Markus Krausz, Tobias Oder, Julian Speith · International Conference on Electronics, Circuits, and Systems 2018 · https://doi.org/10.1109/icecs.2018.8617969 · relevance: low · basis: abstract

**Problem:** Lattice-based cryptography is a promising candidate and remedy in public-key cryptography in case quantum computers become feasible or a major breakthrough in solving the factorization problem or the discrete logarithm problem is achieved.
**Approach:** In this work we discuss the potential of lattice-based signature schemes for practical applications on constrained devices in a post-quantum era. We focus on the schemes GLP, BLISS, and Dilithium and discuss their unique properties as well as challenges regarding their implementation on embedded devices.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** In this regard we present and review optimized implementations of these schemes for ARM Cortex-M4 microcontrollers to evaluate the practical performance of the schemes.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — post-quantum/hybrid authentication scheme; the engine implements PQ and hybrid signatures, but the abstract states no SCMS-scale or traffic-coupled evaluation.
**Novelty implications:** We cannot claim PQ authentication for vehicles is new; the end-to-end safety cost of PQ in a full SCMS remains open as far as this abstract shows.
