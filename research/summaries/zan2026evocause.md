# EvoCause: LLM-Guided Evolution of Causal Graphs for Root Cause Analysis
Lei Zan, Keli Zhang, Shifeng Xie et al. · arXiv 2026 · https://arxiv.org/abs/2607.27290 · relevance: low · basis: abstract

**Problem:** Modern telecommunication, cloud, and microservice systems emit correlated alarm cascades when components fail.
**Approach:** A common approach learns a causal graph from observational logs and predicts all zero-in-degree alarms in each incident-induced subgraph. EvoCause uses a large language model (LLM) to propose semantically plausible graph edits, while deterministic code validates node identities and acyclicity and retains the best graph on a labeled alignment set.
**Tools and scale:** Not stated in the abstract.
**Results:** On synthetic data, EvoCause initialized with the PC causal discovery algorithm outperforms the unrefined PC baseline, raising Node F1, Case EM, and Graph F1 by $11.59$, $9.40$, and $4.59$ percentage points, respectively, while reducing nSHD by $0.2379$.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
