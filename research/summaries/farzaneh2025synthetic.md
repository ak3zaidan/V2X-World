# Synthetic Counterfactual Labels for Efficient Conformal Counterfactual Inference
Amirmohammad Farzaneh, Matteo Zecchin, Osvaldo Simeone · arXiv.org 2025 · https://arxiv.org/abs/2509.04112 · relevance: medium · basis: abstract

**Problem:** Conformal counterfactual inference intervals are too conservative when counterfactual samples are scarce.
**Approach:** SP-CCI augments calibration with synthetic counterfactual labels from a pre-trained model, with RCPS and prediction-powered debiasing.
**Tools and scale:** Theory plus several datasets.
**Results:** Tighter intervals with preserved marginal coverage.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** partial — relevant method for direction 3 (counterfactual prediction with guarantees); we have exact counterfactuals instead of synthetic ones.
**Novelty implications:** A simulator with exact per-unit counterfactuals could serve as ground truth to validate such CCI methods; that use remains open.
