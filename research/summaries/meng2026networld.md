# NetWorld: Communication-Based Diffusion World Model for Multi-Agent Reinforcement Learning in Wireless Networks
Kechen Meng, Rongpeng Li, Yansha Deng, Zhifeng Zhao et al. · arXiv 2026 · https://arxiv.org/abs/2602.00558 · relevance: medium · basis: abstract

**Problem:** As wireless communication networks grow in scale and complexity, diverse resource allocation tasks become increasingly critical. Multi-Agent Reinforcement Learning (MARL) provides a promising solution for distributed control, yet it often requires costly real-world interactions and lacks generalization across diverse tasks.
**Approach:** Motivated by these challenges and opportunities, we propose a Communication-based Diffusion World Model (NetWorld) to enable few-shot generalization across heterogeneous MARL tasks in wireless networks.
**Tools and scale:** Multi-Agent Reinforcement Learning (MARL) provides a promising solution for distributed control, yet it often requires costly real-world interactions and lacks generalization across diverse tasks.
**Results:** We further introduce a lightweight Mean Field (MF) communication mechanism to reduce non-stationarity and promote coordinated behaviors with low overhead. Experiments on three representative tasks demonstrate improved performance and sample efficiency over MARL baselines, indicating strong scalability and practical potential for wireless network optimization.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — Learned wireless/PHY model (foundation or world model); no V2X security, PKI or closed-loop traffic.
**Novelty implications:** Shows learned radio surrogates are active work; a surrogate trained on joint traffic-radio-PKI ground truth with counterfactual labels is not addressed.
