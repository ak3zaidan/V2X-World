# CausalSim: A Causal Framework for Unbiased Trace-Driven Simulation
Abdullah Alomar, Pouya Hamadanian, Arash Nasr-Esfahany, Anish Agarwal, Mohammad Alizadeh, Devavrat Shah · 20th USENIX Symposium on Networked Systems Design and Implementation (2023) 1115--1147 2022 · https://arxiv.org/abs/2201.01811 · relevance: high · basis: full text (extracted sections)

**Problem:** Trace-driven simulators assume traces are unaffected by the simulated intervention, but traces are biased by the algorithms in use during collection.
**Approach:** CausalSim learns a causal model of system dynamics and latent factors from an RCT and casts unbiased trace-driven simulation as sparse tensor completion exploiting distributional invariance.
**Tools and scale:** Real and synthetic data including more than ten months from the Puffer video-streaming system (ABR).
**Results:** Reduces simulation error by 53% and 61% on average vs expert-designed and supervised baselines.
**Limitations and future work (as stated by the authors):**
- Analytical recovery method relies on exact low rank, is brittle with approximate rank, applies only to discrete actions, and gives sufficient but not necessary conditions.
- Requires an initial randomized control trial.
**Overlap with our engine:** partial - counterfactual simulation in networking (direction 3); our engine instead produces exact counterfactuals by same-seed reruns.
**Novelty implications:** We cannot claim to be the first to pose counterfactual/causal network simulation. Open: exact per-unit counterfactual ground truth in V2X to benchmark such causal estimators.
