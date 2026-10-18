# Beyond Computation: A Fragmentation-Aware Performance Analysis of Cryptographic Schemes for Real-Time C-V2X Cooperative Perception
Mason Minear, Mike Haggart, Junaid Khan · 2026 International Conference on Computing, Networking and Communications (ICNC) 2026 · https://doi.org/10.1109/icnc68183.2026.11417009 · relevance: high · basis: abstract

**Problem:** Crypto analyses for C-V2X cooperative perception ignore network effects such as fragmentation.
**Approach:** Simulation of AES, ECC, RSA and homomorphic encryption under congestion, including fragmentation. (summarised from the abstract only)
**Tools and scale:** High-fidelity C-V2X simulation under congestion (simulator not named in abstract).
**Results:** HE ciphertext expansion causes fragmentation and catastrophic end-to-end latency; network-layer constraints, not compute, are the main limiter.
**Limitations and future work (as stated by the authors):**
- Not stated.
**Overlap with our engine:** partial — network-layer (fragmentation) cost of crypto in C-V2X, a central argument of direction 2.
**Novelty implications:** The 'network not compute is the bottleneck' point is already published for CPMs; our contribution must go beyond it (PQ signatures, SCMS, safety outcomes).
