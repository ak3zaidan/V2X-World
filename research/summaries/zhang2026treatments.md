# When Treatments Spill Over: A Neural Framework for Spatiotemporal Causal Effect Estimation
Shaohui Zhang, Jiasheng Shi, Jing Huang · arXiv 2026 · https://arxiv.org/abs/2610.16086 · relevance: medium · basis: abstract

**Problem:** Real-world public health policies are often enacted by local jurisdictions, but their effects can ripple through connected populations. For causal evaluation, this creates a difficult spatiotemporal problem: intermittent policy updates create path-level treatments, and because spillover depends on neighboring treatment paths along with the focal-unit's own ...
**Approach:** To address this challenge, we develop a neural spatiotemporal spillover framework that extends potential-outcome causal analysis to networked policy-systems with unit-level effects and structured interference.
**Tools and scale:** Real-world public health policies are often enacted by local jurisdictions, but their effects can ripple through connected populations.
**Results:** Beyond theoretical guarantees, simulations show improved performance over outcome-regression baselines; in a practically motivated county-level COVID-19 policy study, the framework finds both business-and-economic restrictions and education-and-childcare policies associated with ...
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Causal inference under interference, methodologically relevant to counterfactual reruns; no simulator or V2X content.
**Novelty implications:** Provides estimands/identification theory for spillover effects; exact simulator-generated per-unit counterfactuals for V2X remain unaddressed here.
