# SMART: Scalable Multi-agent Real-time Simulation via Next-token Prediction
Wei Wu, Xiaoxin Feng, Zi-Yan Gao, Yuheng Kan · arXiv.org 2024 · https://arxiv.org/abs/2405.15677 · relevance: low · basis: abstract

**Problem:** Data-driven autonomous driving motion generation tasks are frequently impacted by the limitations of dataset size and the domain gap between datasets, which precludes their extensive application in real-world scenarios.
**Approach:** To address this issue, we introduce SMART, a novel autonomous driving motion generation paradigm that models vectorized map and agent trajectory data into discrete sequence tokens. These tokens are then processed through a decoder-only transformer architecture to train for the next token prediction task across spatial-temporal series.
**Tools and scale:** Mentioned in abstract: dataset, emulated, simulation.
**Results:** SMART achieves state-of-the-art performance across most of the metrics on the generative Sim Agents challenge, ranking 1st on the leaderboards of Waymo Open Motion Dataset (WOMD), demonstrating remarkable inference speed. Moreover, SMART represents the generative model in the autonomous driving motion domain, exhibiting zero-shot generalization capabilities: Using only the NuPlan dataset for training and WOMD for validation, SMART achieved a competitive score of 0.72 on the Sim Agents challenge.
**Limitations and future work (as stated by the authors):**
- Data-driven autonomous driving motion generation tasks are frequently impacted by the limitations of dataset size and the domain gap between datasets, which precludes their extensive application in real-world scenarios.
**Overlap with our engine:** partial — touches network/traffic simulation but not the joint traffic/radio/SCMS closed loop.
**Novelty implications:** Nothing in our candidate directions is ruled out; useful only as background on network/traffic simulation.
