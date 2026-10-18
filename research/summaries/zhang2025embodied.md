# Embodied AI-Enhanced Vehicular Networks: An Integrated Large Language Models and Reinforcement Learning Method
Ruichen Zhang, Changyuan Zhao, Hongyang Du et al. · arXiv 2025 · https://arxiv.org/abs/2501.01141 · relevance: low · basis: abstract

**Problem:** This paper investigates adaptive transmission strategies in embodied AI-enhanced vehicular networks by integrating large language models (LLMs) for semantic information extraction and deep reinforcement learning (DRL) for decision-making.
**Approach:** The proposed framework aims to optimize both data transmission efficiency and decision accuracy by formulating an optimization problem that incorporates the Weber-Fechner law, serving as a metric for balancing bandwidth utilization and quality of experience (QoE). Specifically, we employ the large language and vision assistant (LLAVA) model to extract critical semantic information from raw image data captured by embodied AI agents (i.e., vehicles), reducing transmission data size by approximately more than 90\% while retaining essential content for vehicular communication and decision-making.
**Tools and scale:** Mentions: reinforcement learning.
**Results:** Simulation results show that attention maps from LLAVA highlight the model's focus on relevant image regions, enhancing semantic representation accuracy. Additionally, our proposed transmission strategy improves QoE by up to 36\% compared to DDPG and accelerates convergence by reducing required steps by up to 47\% compared to pure PPO.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
