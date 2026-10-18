# Large Wireless Localization Model (LWLM): A Foundation Model for Positioning in 6G Networks
Guangjin Pan, Kaixuan Huang, Hui Chen, Shunqing Zhang et al. · IEEE Transactions on Wireless Communications 2025 · https://doi.org/10.1109/twc.2026.3718678 · relevance: medium · basis: abstract

**Problem:** Accurate and robust localization is a critical enabler for emerging 5G and 6G applications, including autonomous driving, extended reality (XR), and smart manufacturing. While data-driven approaches have shown promise, most existing models require large amounts of labeled data and struggle to generalize across deployment scenarios and wireless configurations.
**Approach:** To address these limitations, we propose a foundation-model-based solution tailored for wireless localization. We first analyze how different self-supervised learning (SSL) tasks acquire general-purpose and task-specific semantic features based on information bottleneck (IB) theory.
**Tools and scale:** Mentioned in abstract: 5G, 6G. (From the abstract only.)
**Results:** Specifically, we propose an SSL framework that jointly optimizes three complementary objectives: 1) spatial-frequency masked channel modeling (SF-MCM), 2) domain-transformation invariance (DTI), and 3) position-invariant contrastive learning (PICL). In particular, LWLM achieves 26.0%–87.5% improvement over transformer models without pretraining, and exhibits strong generalization under label-limited fine-tuning, unseen BS configurations and cross-dataset evaluations, confirming its potential as a foundation model for wireless localization.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** none — Adjacent background (foundation/pretrained model for wireless signals/channels); no direct overlap with the engine's traffic+radio+SCMS joint simulation.
**Novelty implications:** Does not restrict any candidate claim.
