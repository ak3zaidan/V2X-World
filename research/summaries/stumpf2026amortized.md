# Amortized Counterfactual Prediction with Graph Neural Networks
Hendrik Stumpf, Francesco Quinzan · arXiv 2026 · https://arxiv.org/abs/2610.13724 · relevance: low · basis: abstract

**Problem:** Counterfactual inference asks how an observed outcome might have differed under different circumstances, such as whether a patient would have recovered had they received a different treatment.
**Approach:** A common approach is therefore to first estimate the causal graph from observational data and then perform counterfactual inference. To address this challenge, we introduce the Counterfactual Graph Neural Process (CFGNP), an end-to-end approach that answers counterfactual queries directly from observational data without first estimating a causal graph.
**Tools and scale:** Not stated in the abstract.
**Results:** Across synthetic and image-based benchmarks, CFGNP achieves stronger predictive performance than existing approaches, including on the German Loan benchmark, where it reduces MSE from $4.51 \times 10^{-2}$ to $0.19 \times 10^{-2}$.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
