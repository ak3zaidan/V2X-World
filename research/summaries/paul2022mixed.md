# Mixed Certificate Chains for the Transition to Post-Quantum Authentication in TLS 1.3
Sebastian Paul, Yulia Kuzovkova, Norman Lahr, R. Niederhagen · IACR Cryptology ePrint Archive 2022 · https://doi.org/10.1145/3488932.3497755 · relevance: low · basis: abstract

**Problem:** Large-scale quantum computers will be able to efficiently solve the underlying mathematical problems of widely deployed public key cryptosystems in the near future.
**Approach:** In this work, we propose and investigate a migration strategy towards post-quantum (PQ) authentication for the network protocol Transport Layer Security (TLS). In order to demonstrate the feasibility of our migration strategy, we combine the well-studied and trusted hash-based signature schemes SPHINCS+ and XMSS with elliptic curve cryptography first and subsequently with lattice-based PQC signature schemes (CRYSTALS-Dilithium and Falcon).
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** Furthermore, we combine authentication based on mixed certificate chains with the lattice-based key encapsulation mechanism (KEM) CRYSTALS-Kyber as representative for PQC KEMs to evaluate a fully post-quantum and mutually authenticated TLS 1.3 handshake. Our results show that mixed certificate chains containing hash-based signature schemes only at the root certificate authority level lead to feasible connection establishment times despite the increase in communication size.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — post-quantum/hybrid authentication scheme; the engine implements PQ and hybrid signatures, but the abstract states no SCMS-scale or traffic-coupled evaluation.
**Novelty implications:** We cannot claim PQ authentication for vehicles is new; the end-to-end safety cost of PQ in a full SCMS remains open as far as this abstract shows.
