# Parameter Privacy-Preserving Data Sharing: A Particle-Belief MDP Formulation
Haokun Yu, Jingyuan Zhou, Kaidi Yang · arXiv 2026 · https://arxiv.org/abs/2602.04262 · relevance: low · basis: abstract

**Problem:** This paper investigates parameter-privacy-preserving data sharing in continuous-state dynamical systems, where a data owner designs a data-sharing policy to support downstream estimation and control while preventing adversarial inference of a sensitive parameter.
**Approach:** To efficiently characterize information-theoretic privacy leakage in continuous state and action spaces, we propose a particle-belief MDP formulation that tracks the parameter posterior via sequential Monte Carlo, yielding a tractable belief-state approximation that converges asymptotically as the number of particles increases.
**Tools and scale:** Not stated in the abstract.
**Results:** We show that this problem admits an equivalent belief Markov decision process (MDP) formulation, which provides a simplified representation of the optimal policy. Experiments on a mixed-autonomy platoon show that the learned continuous policy substantially impedes inference attacks on human-driving behavior parameters while maintaining data usability and system performance.
**Limitations and future work (as stated by the authors):**
- None stated in the available text.
**Overlap with our engine:** none — adjacent background; does not implement simulation, SCMS backend, or closed-loop evaluation comparable to the engine (judged from abstract).
**Novelty implications:** Rules out no candidate-direction claim on the available evidence; background reference only.
