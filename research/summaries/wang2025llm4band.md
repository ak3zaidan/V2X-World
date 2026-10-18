# LLM4Band: Enhancing Reinforcement Learning with Large Language Models for Accurate Bandwidth Estimation
Zhijian Wang, Rongwei Lu, Zhiyang Zhang, Cedric Westphal et al. · Proceedings of the 35th edition of the Workshop on Network and Operating System Support for Digital Audio and Video 2025 · https://doi.org/10.1145/3712678.3721880 · relevance: medium · basis: abstract

**Problem:** Real-time communication (RTC) applications rely on accurate bandwidth estimation to ensure high-quality communication and user experience. Traditional heuristic and reinforcement learning (RL)-based methods often face challenges with the dynamic nature of real-time networks, leading to issues with generalization.
**Approach:** Inspired by the success of Large Language Models (LLMs)---which, with billions of parameters pre-trained on massive datasets, have demonstrated exceptional capabilities in semantic representation, adaptability, and transfer learning---we propose LLM4Band, a novel framework that integrates LLMs with offline reinforcement learning to tackle bandwidth estimation in RTC scenarios.
**Tools and scale:** Mentioned in abstract: LLM, reinforcement learning. (From the abstract only.)
**Results:** By leveraging the powerful feature extraction capabilities of LLMs and combining them with an offline RL algorithm, LLM4Band incorporates a Balanced Replay Buffer and an LLM-based policy network to significantly enhance robustness and adaptability. Extensive experiments demonstrate that LLM4Band surpasses state-of-the-art methods, achieving a 12.35% improvement in estimation accuracy and a 21% enhancement in communication quality.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** none — Adjacent background (LLM applied to wireless/vehicular networking); no direct overlap with the engine's traffic+radio+SCMS joint simulation.
**Novelty implications:** Does not restrict any candidate claim.
