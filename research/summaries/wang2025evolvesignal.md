# EvolveSignal: A Large Language Model Powered Coding Agent for Discovering Traffic Signal Control Strategies
Leizhen Wang, Peibo Duan, Hao Wang et al. · arXiv 2025 · https://arxiv.org/abs/2509.03335 · relevance: low · basis: abstract

**Problem:** In traffic engineering, fixed-time traffic signal control remains widely used for its low cost, stability, and interpretability.
**Approach:** Rather than deriving entirely new analytical formulations, the proposed framework focuses on exploring code-level variations of existing control logic and identifying effective combinations of heuristic modifications. Experiments on a signalized intersection demonstrate that the discovered strategies outperform a classical baseline (Webster's method), reducing average delay by 20.1\% and average stops by 47.1\%.
**Tools and scale:** Not stated in the abstract.
**Results:** However, its design relies on hand-crafted formulas (e.g., Webster) and manual re-timing by engineers to adapt to demand changes, which is labor-intensive and often yields suboptimal results under heterogeneous or congested conditions.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
