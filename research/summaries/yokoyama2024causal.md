# Causal-discovery-based root-cause analysis and its application in time-series prediction error diagnosis
Hiroshi Yokoyama, Ryusei Shingaki, Kaneharu Nishino et al. · arXiv 2024 · https://arxiv.org/abs/2411.06990 · relevance: low · basis: abstract

**Problem:** Recent rapid advancements of machine learning have greatly enhanced the accuracy of prediction models, but most models remain "black boxes", making prediction error diagnosis challenging, especially with outliers.
**Approach:** To address these limitations, we introduce the Causal-Discovery-based Root-Cause Analysis (CD-RCA) method that estimates causal relationships between the prediction error and the explanatory variables, without needing a pre-defined causal graph.
**Tools and scale:** Not stated in the abstract.
**Results:** Extensive experiments show CD-RCA outperforms current heuristic attribution methods.
**Limitations and future work (as stated by the authors):**
- "To address these limitations, we introduce the Causal-Discovery-based Root-Cause Analysis (CD-RCA) method that estimates causal relationships between the prediction error and the explanatory variables, without needing a pre-defined causal graph."
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
