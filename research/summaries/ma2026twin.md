# Twin Rollouts: Noise-Coupled Counterfactual Branching in Interactive Video World Models
Yu Ma, Hongli Shi, Xinran Xu · venue unknown 2026 · https://arxiv.org/abs/2608.08982 · relevance: medium · basis: abstract

**Problem:** Interactive video world models generate rollouts autoregressively under an action stream, yet they are trained and evaluated almost exclusively on factual prediction. We study counterfactual generation inside the rollout: given a trajectory the model has itself generated, what would have happened had the actions differed from step t* onward?
**Approach:** We formalize noise-coupled twin rollouts --- a factual and a counterfactual branch sharing the generated prefix and the future exogenous noise sequence, diverging only in the action stream at an intervention point. Noise coupling further turns the minimal-change principle into a per-sample verifiable property: we define a spatiotemporal locality metric that penalizes divergence outside the causal descendants of the intervention, computable against simulator ground truth without a learned judge.
**Tools and scale:** Not stated in the abstract. (From the abstract only.)
**Results:** No quantitative results stated in the abstract.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** partial — Noise-coupled twin rollouts: factual vs counterfactual branches sharing exogenous noise in learned world models.
**Novelty implications:** Counterfactual branching with shared noise is formalized for video world models; doing it with an exact simulator in V2X is open.
