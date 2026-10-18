# Toward 6G Authentication and Key Agreement Protocol: A Survey on Hybrid Post Quantum Cryptography
T. Turnip, Birger Andersen, C. Vargas-Rosales · IEEE Communications Surveys and Tutorials 2026 · https://doi.org/10.1109/comst.2025.3567439 · relevance: low · basis: abstract

**Problem:** The evolution of 6G networks necessitates robust authentication and key agreement (AKA) protocols to counter emerging security challenges, particularly the threat posed by quantum computing.
**Approach:** We evaluate NIST-standardized PQC algorithms, focusing on their computational overhead, key size efficiency, and adaptability to heterogeneous network environments, including IoT and ultra-low latency applications. Furthermore, we propose a quantum-resistant 6G-AKA framework, integrating hybrid PQC, AI-driven trust mechanisms, and decentralized authentication to ensure scalability and interoperability.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** This study examines the vulnerabilities of 5G-AKA and explores hybrid post-quantum cryptography (PQC) as a transitional security solution for 6G networks. A comparative analysis of hybrid cryptographic schemes demonstrates that lattice-based (Kyber, Dilithium), hash-based (SPHINCS+), and code-based (BIKE, Classic McEliece) PQC techniques provide varying trade-offs between security, efficiency, and deployment feasibility.
**Limitations and future work (as stated by the authors):**
- "Existing 5G-AKA protocols remain vulnerable to user tracking, replay attacks, and false base station impersonation, while Elliptic Curve Cryptography (ECC) and RSA-based key exchanges face imminent obsolescence due to quantum attacks."
- "Experimental benchmarks highlight potential performance constraints, including latency in PQC-based key exchange and resource limitations in edge computing."
**Overlap with our engine:** partial — post-quantum/hybrid authentication scheme; the engine implements PQ and hybrid signatures, but the abstract states no SCMS-scale or traffic-coupled evaluation.
**Novelty implications:** We cannot claim PQ authentication for vehicles is new; the end-to-end safety cost of PQ in a full SCMS remains open as far as this abstract shows.
