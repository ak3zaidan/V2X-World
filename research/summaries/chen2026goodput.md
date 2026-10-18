# Goodput Maximization for Large Language Model Edge Inference: A Two-Phase Maskable PPO Approach
Xiaojing Chen, Qi Zhang, Wei Ni et al. · arXiv 2026 · https://doi.org/10.1109/lwc.2026.3718001 · relevance: low · basis: abstract

**Problem:** This paper presents a novel two-phase maskable proximal policy optimization (TP-MPPO) algorithm, which maximizes the system goodput counting request throughput with strict service level objective (SLO) compliance for large language model (LLM) inference services in wireless edge networks.
**Approach:** In the first phase of TP-MPPO, we optimize the task offloading decisions by MPPO with action masking mechanism, effectively avoiding exploring invalid actions and reducing the action space.
**Tools and scale:** Not stated in the abstract.
**Results:** Simulation results demonstrate that TP-MPPO can improve the system reward by 33.3%--87.5% compared to its benchmarks and achieve the highest goodput.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
