# Yet Another Traffic Classifier: A Masked Autoencoder Based Traffic Transformer with Multi-Level Flow Representation
Rui-Jie Zhao, Ming-Wei Zhan, Xian-Wen Deng, Yanhao Wang et al. · AAAI Conference on Artificial Intelligence 2023 · https://doi.org/10.1609/aaai.v37i4.25674 · relevance: medium · basis: abstract

**Problem:** Traffic classification is a critical task in network security and management. Recent research has demonstrated the effectiveness of the deep learning-based traffic classification method.
**Approach:** In this paper, we introduce a masked autoencoder (MAE) based traffic transformer with multi-level flow representation to tackle these problems. To model raw traffic data, we design a formatted traffic representation matrix with hierarchical flow information.
**Tools and scale:** Mentioned in abstract: real-world. (From the abstract only.)
**Results:** However, the following limitations remain: (1) the traffic representation is simply generated from raw packet bytes, resulting in the absence of important information; (2) the model structure of directly applying deep learning algorithms does not take traffic characteristics into account; and (3) scenario-specific classifier training usually requires a labor-intensive and time-consuming process to label data. Experiment findings reveal that our method outperforms state-of-the-art methods on five real-world traffic datasets by a large margin.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** none — Adjacent background (foundation/pretrained model for network traffic); no direct overlap with the engine's traffic+radio+SCMS joint simulation.
**Novelty implications:** Does not restrict any candidate claim.
