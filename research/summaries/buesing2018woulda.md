# Woulda, Coulda, Shoulda: Counterfactually-Guided Policy Search
L. Buesing, T. Weber, Yori Zwols, N. Heess, S. Racanière, A. Guez et al. · International Conference on Learning Representations 2018 · https://arxiv.org/abs/1811.06272 · relevance: low · basis: abstract

**Problem:** Learning policies on data synthesized by models can in principle quench the thirst of reinforcement learning algorithms for large amounts of real experience, which is often costly to acquire.
**Approach:** However, simulating plausible experience de novo is a hard problem for many complex environments, often resulting in biases for model-based policy evaluation and search. Instead of de novo synthesis of data, here we assume logged, real experience and model alternative outcomes of this experience under counterfactual actions, actions that were not actually taken.
**Tools and scale:** Not stated in the abstract.
**Results:** CF-GPS can improve on vanilla model-based RL algorithms by making use of available logged data to de-bias model predictions. We find empirically that these advantages translate into improved policy evaluation and search results on a non-trivial grid-world task.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — adjacent topic (V2X simulation/datasets (direction 3 area)); no integrated traffic/radio/SCMS simulation per the abstract.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract.
