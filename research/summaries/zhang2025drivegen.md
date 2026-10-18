# DriveGen: Towards Infinite Diverse Traffic Scenarios with Large Models
Shenyu Zhang, Jiaguo Tian, Zhengbang Zhu et al. · arXiv 2025 · https://arxiv.org/abs/2503.05808 · relevance: low · basis: abstract

**Problem:** Microscopic traffic simulation has become an important tool for autonomous driving training and testing.
**Approach:** In this paper, we propose DriveGen, a novel traffic simulation framework with large models for more diverse traffic generation that supports further customized designs. DriveGen consists of two internal stages: the initialization stage uses large language model and retrieval technique to generate map and vehicle assets; the rollout stage outputs trajectories with selected waypoint goals from visual language model and a specific designed diffusion planner.
**Tools and scale:** Not stated in the abstract.
**Results:** Experiments show that our generated scenarios and corner cases have a superior performance compared to state-of-the-art baselines.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
