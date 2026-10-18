# Hybrid LLM-DDQN based Joint Optimization of V2I Communication and Autonomous Driving
Zijiang Yan, Hao Zhou, Hina Tabassum et al. · arXiv 2024 · https://arxiv.org/abs/2410.08854 · relevance: low · basis: abstract

**Problem:** Large language models (LLMs) have received considerable interest recently due to their outstanding reasoning and comprehension capabilities.
**Approach:** Such an iterative optimization approach can better explore the interactions between LLMs and conventional reinforcement learning techniques, revealing the potential of using LLMs for network optimization and management. Finally, the simulations demonstrate that our proposed hybrid LLM-DDQN approach outperforms the conventional DDQN algorithm, showing faster convergence and higher average rewards.
**Tools and scale:** Mentions: reinforcement learning.
**Results:** We deploy LLMs for AD decision-making to maximize traffic flow and avoid collisions for road safety, and a double deep Q-learning algorithm (DDQN) is used for V2I optimization to maximize the received data rate and reduce frequent handovers. In particular, for LLM-enabled AD, we employ the Euclidean distance to identify previously explored AD experiences, and then LLMs can learn from past good and bad decisions for further improvement.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
