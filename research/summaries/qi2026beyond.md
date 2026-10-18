# Beyond Static Forecasting: Unleashing the Power of World Models for Mobile Traffic Extrapolation
Xiaoqian Qi, Haoye Chai, Yue Wang, Yong Li · arXiv 2026 · https://arxiv.org/abs/2604.08199 · relevance: medium · basis: abstract

**Problem:** Traffic prediction models cannot capture dynamics under network-parameter changes.
**Approach:** MobiWM world model: cell traffic as state, antenna parameters as actions; rollouts for counterfactual planning.
**Tools and scale:** Large variable-parameter mobile traffic datasets.
**Results:** Outperforms baselines by at least 16.40% on average; actor-critic case study as surrogate.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** partial — action-conditioned network world model for counterfactual planning (direction 3) for cellular traffic, not V2X.
**Novelty implications:** Network world models for counterfactual rollouts exist; validating against exact simulator counterfactuals in V2X remains open.
