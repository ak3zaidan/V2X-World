# Simulating LLM training workloads for heterogeneous compute and network infrastructure
Sumit Kumar, Arjun Temura, Naman Sharma et al. · arXiv 2025 · https://arxiv.org/abs/2508.05370 · relevance: low · basis: abstract

**Problem:** The growing demand for large-scale GPU clusters in distributed model training presents a significant barrier to innovation, particularly in model optimization, performance tuning, and system-level enhancements.
**Approach:** To address the gap between state-of-the-art and practical requirements, we propose the design of a heterogeneity-aware distributed LLM simulator capable of predicting training time while enabling abstractions to specify custom configurations for device groups and device-to-parallelism mapping. We present the design requirements and challenges in building a heterogeneity-aware distributed ML training simulator, and design components such as non-uniform workload partitioning.
**Tools and scale:** Not stated in the abstract.
**Results:** Our initial simulation results demonstrate the impact of heterogeneity on the model computation and communication time.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
