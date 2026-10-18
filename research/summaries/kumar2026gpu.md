# GPU-Accelerated Post-Quantum OTA Key Exchange Using Batch-Optimized ML-KEM
N. L. Ujwal Kumar, Digvijay Shinde, M. Arun · IEEE Access 2026 · https://doi.org/10.1109/access.2026.3728882 · relevance: medium · basis: abstract

**Problem:** Secure over-the-air (OTA) firmware updates are indispensable to connected and autonomous vehicles (CAVs), enabling rapid vulnerability patching without physical recall. The impending arrival of cryptographically relevant quantum computers, however, threatens the public-key protocols that protect these updates.
**Approach:** This paper proposes a high-throughput, batch-optimized GPU acceleration architecture for server-side ML-KEM-1024 key exchange while providing a realistic system-level bottleneck evaluation.
**Tools and scale:** Not stated in the abstract; appears to be analytical/cryptographic or review work.
**Results:** By pushing isolated GPU polynomial arithmetic to a peak mathematical speedup of 92.7x (164 GOPS) at N = 32,768 clients, we definitively eliminate the lattice-math bottleneck to expose the system’s true limitation: unaccelerated CPU Keccak hashing, which our comprehensive profiling reveals dominates 95% of total execution time. PCIe data transfer overhead remains minimal at 0.027% of the total pipeline.
**Limitations and future work (as stated by the authors):**
- By pushing isolated GPU polynomial arithmetic to a peak mathematical speedup of 92.7x (164 GOPS) at N = 32,768 clients, we definitively eliminate the lattice-math bottleneck to expose the system’s true limitation: unaccelerated CPU Keccak hashing, which our comprehensive profiling reveals dominates ...
- PCIe data transfer overhead remains minimal at 0.027% of the total pipeline.
**Overlap with our engine:** partial — post-quantum ML-KEM OTA key exchange performance.
**Novelty implications:** Does not remove any of our claims; it is a related point of comparison. We cannot claim to be first on the specific mechanism it covers; the joint closed-loop, multi-layer evaluation remains open as far as this abstract shows.
