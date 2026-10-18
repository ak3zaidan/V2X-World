# Estimating counterfactual treatment outcomes over time in complex multiagent scenarios
Keisuke Fujii, Koh Takeuchi, Atsushi Kuribayashi et al. · arXiv 2022 · https://arxiv.org/abs/2206.01900 · relevance: low · basis: abstract

**Problem:** Evaluation of intervention in a multiagent system, e.g., when humans should intervene in autonomous driving systems and when a player should pass to teammates for a good shot, is challenging in various engineering and scientific fields.
**Approach:** Here we propose an interpretable, counterfactual recurrent network in multiagent systems to estimate the effect of the intervention. Our model leverages graph variational recurrent neural networks and theory-based computation with domain knowledge for the ITE estimation framework based on long-term prediction of multiagent covariates and outcomes, which can confirm the circumstances under which the intervention is effective.
**Tools and scale:** Not stated in the abstract.
**Results:** On simulated models of an automated vehicle and biological agents with time-varying confounders, we show that our methods achieved lower estimation errors in counterfactual covariates and the most effective treatment timing than the baselines.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
