# NetCause: Counterfactual Learning for Root Cause Analysis in Large-Scale Networks
Fabien Chraim, Jian Zhang, Dominik Janzing et al. · arXiv 2026 · https://arxiv.org/abs/2606.13543 · relevance: low · basis: abstract

**Problem:** Can a learned model capture how faults propagate through a large-scale network and use this knowledge to causally attribute customer impact to its underlying root cause?
**Approach:** We present NetCause, a self-supervised learning-based framework that models network incidents as graph-temporal processes and uses counterfactual simulation to rank candidate root causes. This approach produces an interpretable ranking of root cause hypotheses and integrates naturally with operator-defined mitigation and remediation actions.
**Tools and scale:** Not stated in the abstract.
**Results:** NetCause consistently improves root cause ranking quality in the regime most relevant to operational decision-making, achieving a 16.1% accuracy improvement over a rule-based heuristic baseline.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
