# Benchmarking Hybrid Quantum-Safe Cryptography Over Real-World Infrastructures
Jaime S. Buruaga, Juan P. Brito, Vicente Martin · EPJ Quantum Technology 2025 · https://doi.org/10.2139/ssrn.5733337 · relevance: low · basis: abstract

**Problem:** The two leading technologies to address this challenge are Post-Quantum Cryptography and Quantum Key Distribution.
**Approach:** In this work, we present the first long-running test with detailed measurements of a suite of hybrid quantum-safe protocols that integrate classical cryptography, PQC, and QKD within a production environment (MadQCI). Using standardized protocols and interfaces and employing Transport Layer Security (TLS) as the base protocol for real-world comparison, this study demonstrates the efficiency and scalability of the proposed solution in deployed heterogeneous networks.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** However, changing current network protocols to quantum-safe alternatives is not just a matter of security: their efficiency and scalability must also be demonstrated within real-world telecommunications infrastructures to ensure their practical adoption.
**Limitations and future work (as stated by the authors):**
- "A promising approach is the hybridization of these technologies - combining their respective strengths while mitigating their limitations - and integrating them with the cryptographic mechanisms used today to enhance crypto-agility and facilitate transitioning to a quantum-safe infrastructure."
**Overlap with our engine:** partial — post-quantum/hybrid authentication scheme; the engine implements PQ and hybrid signatures, but the abstract states no SCMS-scale or traffic-coupled evaluation.
**Novelty implications:** We cannot claim PQ authentication for vehicles is new; the end-to-end safety cost of PQ in a full SCMS remains open as far as this abstract shows.
