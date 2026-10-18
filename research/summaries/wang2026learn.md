# Learn for Variation: Efficient AAV Trajectory Learning through a Differentiable Wireless World Model
Xiucheng Wang, Zhenye Chen, Nan Cheng et al. · arXiv 2026 · https://arxiv.org/abs/2603.18853 · relevance: low · basis: abstract

**Problem:** Autonomous aerial vehicles (AAVs) enable data collection for sixth-generation Internet-of-Things networks, but their trajectories couple nonlinear wireless rates with long-horizon service progress.
**Approach:** This paper views the evolution of AAV kinematics, channel state, and user backlog as a structured differentiable world model and develops Learn for Variation (L4V) to exploit that model efficiently. The framework also learns shared OFDMA allocation under reparameterized shadowing and Rician fading, while distributional pretraining amortizes model-based optimization into forward-only deployment on unseen layouts.
**Tools and scale:** Not stated in the abstract.
**Results:** The resulting derivative is exact conditional on a fixed exogenous-noise realization; stochastic expected-objective optimization still requires sampling. We show that the structured adjoint grows at most polynomially with the horizon and establish a stationary-point rate for fixed-step full-gradient descent under standard smoothness assumptions.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — Adjacent background (world model for driving/vehicular systems); does not touch the engine's core layers.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
