# Pedestrian-Aware LLM-Driven Behavioral Planning for Autonomous Vehicles
Aidana Baimbetova, Haruki Yonekura, Hamada Rizk et al. · arXiv 2026 · https://arxiv.org/abs/2605.16858 · relevance: low · basis: abstract

**Problem:** Autonomous Vehicles (AVs) must make reliable decisions in dense urban environments where pedestrian behavior is variable, sometimes abnormal, and often unseen during training.
**Approach:** To address these limitations, we introduce a Large Language Model (LLM)-based decision-making framework for pedestrian-aware behavioral planning. We evaluate the framework in SUMO across multiple pedestrian-interaction scenarios, including unexpected jaywalking, turn-back crossing, hesitation, and bidirectional crossing.
**Tools and scale:** Mentions: SUMO.
**Results:** In zero-shot evaluation, the LLM-based agent achieves a 68% collision-free success rate, substantially outperforming deep RL baselines (17.7%). With few-shot episodic memory in a single-pedestrian scenario, performance increases to 96.0%, exceeding a custom DQN controller (82.0%).
**Limitations and future work (as stated by the authors):**
- "To address these limitations, we introduce a Large Language Model (LLM)-based decision-making framework for pedestrian-aware behavioral planning."
**Overlap with our engine:** none — Adjacent background (LLM agent that drives or generates network/traffic simulations); does not touch the engine's core layers.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
