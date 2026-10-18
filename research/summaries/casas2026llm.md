# LLM-Driven Automated Reward Design for Reinforcement Learning-Based Routing in LEO Satellite Networks
Walter P. Casas, Nelson L. S. da Fonseca, and Carlos A. Astudillo · arXiv 2026 · https://arxiv.org/abs/2608.01649 · relevance: low · basis: abstract

**Problem:** Routing in Low Earth Orbit (LEO) satellite networks is challenging due to highly dynamic topologies and spatio-temporal network conditions.
**Approach:** Reinforcement Learning (RL) has emerged as a promising approach for adaptive routing; however, its performance critically depends on reward function design, which must balance objectives such as goodput and end-to-end delay. We propose LARGE, a framework that automates reward design for RL-based routing by combining LLM- driven generation with iterative simulator-in-the-loop evaluation.
**Tools and scale:** Not stated in the abstract.
**Results:** Results show that LARGE improves reward quality within a few iterations through feedback-driven refinement. Across different backbones, the framework achieves performance comparable to an expert-designed baseline, with the best-performing configuration reaching goodput within approximately 3% of the baseline and slightly lower end-to-end delay, without manual reward engineering.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
