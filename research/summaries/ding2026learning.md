# Learning for Accelerated Traffic Engineering With Differentiable Network Modeling
Wenlong Ding, Libin Liu, Li Chen, Hong Xu · IEEE Transactions on Network Science and Engineering 2026 · https://doi.org/10.1109/tnse.2025.3579938 · relevance: low · basis: abstract

**Problem:** Traffic engineering (TE) in the wide-area networks (WAN) is crucial for optimizing network performance by distributing traffic across various paths.
**Approach:** Traditional methods use optimization like linear programs (LP) to solve TE, resulting in long decision-making time especially in larger topologies due to the iterative nature of optimization algorithms. Deep neural networks (DNNs) have been recently applied to TE to expedite decision-making through model inference.
**Tools and scale:** Mentioned in abstract: simulator, simulators.
**Results:** With dNE, experiments on four DNN-based TE algorithms show that DNNs trained with metric gradients reduce performance loss by over 10x compared to RL, speed up LP solvers by 13000x, and achieve 1000x faster metric computation than traditional event-based simulators.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — touches network/traffic simulation but not the joint traffic/radio/SCMS closed loop.
**Novelty implications:** Nothing in our candidate directions is ruled out; useful only as background on network/traffic simulation.
