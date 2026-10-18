# Advancing Network Digital Twin Framework for Generating Realistic Datasets
Oscar Stenhammar, Sundeep Rangan, Gábor Fodor, Carlo Fischione · arXiv 2026 · https://arxiv.org/abs/2604.12888 · relevance: high · basis: full text (extracted sections)

**Problem:** Measurement campaigns are costly; reproducible data generation is needed.
**Approach:** Open NDT framework integrating controllable vehicular mobility, Sionna ray tracing and ns-3, extracting cross-layer metrics; dataset released.
**Tools and scale:** Sionna, ns-3; vehicular and urban scenarios.
**Results:** Framework and dataset released; runtime scales roughly O(VB).
**Limitations and future work (as stated by the authors):**
- No quantitative comparison with real-world data yet (future measurement campaign).
- Ray tracing increases computational load; GPU recommended.
**Overlap with our engine:** partial - open cross-layer vehicular dataset generator for ML benchmarking (direction 3 benchmark idea), without security or counterfactual pairing.
**Novelty implications:** Simulator-generated vehicular ML datasets exist; per-unit counterfactual ground truth with PKI does not.
