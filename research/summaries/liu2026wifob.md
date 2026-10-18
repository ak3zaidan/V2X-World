# WiFo-INR: A Wireless Foundation Model Based on Implicit Neural Representations
Boxun Liu, Xuanyu Liu, Shijian Gao, Xiang Cheng et al. · arXiv 2026 · https://arxiv.org/abs/2608.08033 · relevance: medium · basis: abstract

**Problem:** Wireless foundation models are emerging as a promising paradigm for AI-native physical-layer design. However, existing methods typically model channel state information (CSI) as image-like discrete tensors with generic token decoders that may struggle to capture complex high-frequency variations efficiently and often produce high-dimensional, size-dependent ...
**Approach:** In this paper, we propose WiFo-INR, an implicit neural representation (INR)-based wireless foundation model that represents CSI as a coordinate-conditioned neural function.
**Tools and scale:** Not stated in the abstract.
**Results:** Compared with existing foundation models, WiFo-INR improves channel reconstruction and CSI feedback performance while substantially reducing inference latency. It also transfers efficiently to diverse wireless tasks with minimal fine-tuning overhead and achieves zero-shot generalization to unseen CSI sizes.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — Learned wireless/PHY model (foundation or world model); no V2X security, PKI or closed-loop traffic.
**Novelty implications:** Shows learned radio surrogates are active work; a surrogate trained on joint traffic-radio-PKI ground truth with counterfactual labels is not addressed.
