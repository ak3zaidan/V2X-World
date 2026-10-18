# Efficient implementation of post-quantum digital signatures on Raspberry Pi
Mahmood A. Al-Shareeda, Ahmed Abdul Hassan Ghadban, A. Glass, Esam Mohamed Abdul Hadi et al. · Discover Applied Sciences 2025 · https://doi.org/10.1007/s42452-025-07201-z · relevance: low · basis: abstract

**Problem:** Falcon has compact signatures, but its integration in an embedded context poses a challenge because it uses floating-point arithmetic and Gaussian sampling.
**Approach:** We provide one of the most detailed yet real-world demonstrations of the NIST-PQC lattice-based digital signature schemes, being the Dilithium family: Dilithium2, Dilithium3, and Dilithium5, being implemented and evaluated on a single-embedded platform, Raspberry Pi 4 Model B. We deliver a more deep-seated portrayal, giving details on platform-determined performance optimizations, performance trade-offs, and energy consumption characteristics, identifying Dilithium2 as a well-balanced prospect for quantum-safe deployments in the context of IoT and the edge.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** Our experiments, performed on stress-tested hardware, show that Dilithium is not only as secure as required by post-quantum security standards but also operates reliably in low-resource environments.
**Limitations and future work (as stated by the authors):**
- "An interactive benchmarking toolkit and open-source codebase provide further reproducibility and potential for future work."
**Overlap with our engine:** partial — post-quantum/hybrid authentication scheme; the engine implements PQ and hybrid signatures, but the abstract states no SCMS-scale or traffic-coupled evaluation.
**Novelty implications:** We cannot claim PQ authentication for vehicles is new; the end-to-end safety cost of PQ in a full SCMS remains open as far as this abstract shows.
