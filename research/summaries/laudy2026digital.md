# The Digital Twin Counterfactual Framework: A Validation Architecture for Simulated Potential Outcomes
Olav Laudy · arXiv 2026 · https://arxiv.org/abs/2604.01325 · relevance: medium · basis: abstract

**Problem:** The fundamental problem of causal inference - that the counterfactual outcome for any individual is never observed - has shaped the entire methodology of the field. Every existing approach substitutes assumptions for missing data: ignorability, parallel trends, exclusion restrictions.
**Approach:** This paper proposes the Digital Twin Counterfactual Framework (DTCF): rather than estimating the counterfactual statistically, we simulate it using a digital twin and subject the simulation to a hierarchical validation regime.
**Tools and scale:** We formalize the digital twin simulator as a stochastic mapping within the potential outcomes framework and introduce a hierarchy of twin fidelity assumptions - from marginal fidelity through joint fidelity to structural fidelity - each unlocking a progressively richer class of estimands.
**Results:** No quantitative results stated in the abstract.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** strong — Proposes simulating counterfactuals with a digital twin under a validation hierarchy, conceptually the same idea as our exact counterfactual reruns (direction 3), though domain-general.
**Novelty implications:** Cannot claim the idea of simulator-produced counterfactuals as novel in general; open: V2X instantiation with exact same-seed reruns and per-unit cross-layer ground truth.
