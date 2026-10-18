# An LLM-Powered Cooperative Framework for Large-Scale Multi-Vehicle Navigation
Yuping Zhou, Siqi Lai, Jindong Han et al. · arXiv 2025 · https://arxiv.org/abs/2510.07825 · relevance: low · basis: abstract

**Problem:** The rise of Internet of Vehicles (IoV) technologies is transforming traffic management from isolated control to a collective, multi-vehicle process.
**Approach:** To address these challenges, we propose CityNav, a hierarchical, LLM-powered framework for large-scale multi-vehicle navigation. To enable effective cooperation, we introduce a cooperative reasoning optimization mechanism, in which agents are jointly trained with a dual-reward structure: individual rewards promote per-vehicle efficiency, while shared rewards encourage network-wide coordination and congestion reduction.
**Tools and scale:** Mentions: reinforcement learning.
**Results:** Extensive experiments on four real-world road networks of varying scales (up to 1.6 million roads and 430,000 intersections) and traffic datasets demonstrate that CityNav consistently outperforms nine classical path search and RL-based baselines in city-scale travel efficiency and congestion mitigation. Our results highlight the potential of LLMs to enable scalable, adaptive, and cooperative city-wide traffic navigation, providing a foundation for intelligent, large-scale vehicle routing in complex urban environments.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
