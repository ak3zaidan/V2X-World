# WirelessGPT: A Generative Foundation Model for Multi-Task Integrated Sensing and Communication
Ting-Ting Yang, Ping Zhang, Meng-Fan Zheng, Yuxuan Shi et al. · IEEE Journal on Selected Areas in Communications 2026 · https://doi.org/10.1109/jsac.2025.3640156 · relevance: medium · basis: abstract

**Problem:** This paper presents WirelessGPT, a generative foundation model designed for multi-task learning in integrated sensing and communication (ISAC) systems. Built upon large-scale heterogeneous wireless datasets including Traciverse, Sensiverse, and DeepMIMO, WirelessGPT learns universal spatio-temporal-frequency representations through self-supervised pretraining with masked channel token prediction.
**Approach:** The proposed architecture introduces a multi-scale patch embedding module to capture both local and global channel features, and a triple-axis attention encoder to jointly model temporal, spatial, and frequency-domain dependencies.
**Tools and scale:** Mentioned in abstract: 6G, GPT. (From the abstract only.)
**Results:** Experimental results show that WirelessGPT achieves superior accuracy and generalization under limited labeled data and dynamic ISAC conditions, outperforming traditional and task-specific models in low-SNR and high-mobility scenarios while maintaining efficient inference suitable for edge deployment. By unifying communication and sensing functionalities within a single generative backbone, WirelessGPT establishes a scalable paradigm for AI-native 6G systems, enabling shared representations that support heterogeneous wireless tasks.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** none — Adjacent background (foundation/pretrained model for wireless signals/channels); no direct overlap with the engine's traffic+radio+SCMS joint simulation.
**Novelty implications:** Does not restrict any candidate claim.
