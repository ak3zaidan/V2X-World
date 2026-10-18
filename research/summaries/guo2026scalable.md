# Scalable Pre-Trained Masked Channel Model of Wireless Communications
Jian-Hua Guo, Zhongsheng Deng, Zhen Qiao, Jun-Kai Zhang et al. · IEEE Transactions on Communications 2026 · https://doi.org/10.1109/tcomm.2026.3675420 · relevance: medium · basis: abstract

**Problem:** Deep learning (DL)-based models have been widely applied in wireless communication systems with excellent performance. However, most of these models are task- and scenario-specific, exhibiting limited generalization and contributing to increasing complexity and overhead with their deployment in systems.
**Approach:** Inspired by the emergent capabilities and strong generalization exhibited by large models (LMs), represented by large language models (LLMs), this paper analyzes the differences between existing DL-based wireless communication models and LLMs, proposing a framework for designing LMs tailored to wireless communications. Building upon this framework, we integrate channel-related tasks of the physical layer into a unified pre-training task, i.e., channel completion, and propose a pre-trained masked channel model (MCM) with different parameter scales ranging from 5 million to 1 billion (B), enabling simultaneous solving of channel state information (CSI) feedback, prediction, and estimation.
**Tools and scale:** Mentioned in abstract: LLM. (From the abstract only.)
**Results:** The formulated scaling laws indicate that the proposed MCM with 1B parameter not only shows no sign of performance saturation on the pre-trained task but also has the potential to enhance performance at larger model sizes. Simulation results demonstrate that the proposed MCM outperforms the existing algorithms across various downstream tasks while exhibiting superior cross-task and cross-scenario generalization capabilities in both simulated and realistic scenarios.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** none — Adjacent background (LLM agent that drives or generates network/traffic simulations); no direct overlap with the engine's traffic+radio+SCMS joint simulation.
**Novelty implications:** Does not restrict any candidate claim.
