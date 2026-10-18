# Efficient Post-Quantum TLS Handshakes using Identity-Based Key Exchange from Lattices
Utsav Banerjee, Anantha P. Chandrakasan · ICC 2020 - 2020 IEEE International Conference on Communications (ICC) 2020 · https://doi.org/10.1109/icc40277.2020.9148829 · relevance: low · basis: abstract

**Problem:** Identity-Based Encryption (IBE) is considered an alternative to traditional certificate-based public key cryptography to reduce communication overheads in wireless sensor networks.
**Approach:** In this work, we build on the well-known lattice-based DLP-IBE scheme to construct an ID-based certificateless authenticated key exchange for post-quantum Transport Layer Security (TLS) handshakes. We also propose concrete parameters for the underlying lattice computations and provide detailed implementation results.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** Finally, we compare the combined computation and communication cost of our ID-based certificate-less handshake with the traditional certificate-based handshake, both using lattice-based algorithms at similar postquantum security levels, and show that our ID-based handshake is $3.7 \times$ more energy-efficient, thus highlighting the advantage of ID-based key exchange for post-quantum TLS.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — post-quantum/hybrid authentication scheme; the engine implements PQ and hybrid signatures, but the abstract states no SCMS-scale or traffic-coupled evaluation.
**Novelty implications:** We cannot claim PQ authentication for vehicles is new; the end-to-end safety cost of PQ in a full SCMS remains open as far as this abstract shows.
