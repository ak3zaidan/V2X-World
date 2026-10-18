# Foundation Models for Wireless Localization: Pretraining, Adaptation, and Utilization
Guangjin Pan, Jiajia Guo, Zheng Xing, Hui Chen et al. · arXiv 2026 · https://arxiv.org/abs/2608.30540 · relevance: medium · basis: abstract

**Problem:** Accurate wireless localization is a key enabler for 6G networks, yet remains challenging under diverse and rapidly changing propagation conditions. Model-based methods degrade when multipath channels are non-resolvable and model mismatches occur, while supervised deep learning demands large labeled datasets and generalizes poorly to new deployments.
**Approach:** Inspired by foundation models (FMs) in language and vision, this article presents a unified framework for FM-based wireless localization that learns transferable channel representations from large-scale unlabeled channel state information and adapts to new environments with minimal or even no supervision.
**Tools and scale:** Not stated in the abstract.
**Results:** Ray-tracing-based case studies show improved positioning accuracy and cross-environment generalization.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — Learned wireless/PHY model (foundation or world model); no V2X security, PKI or closed-loop traffic.
**Novelty implications:** Shows learned radio surrogates are active work; a surrogate trained on joint traffic-radio-PKI ground truth with counterfactual labels is not addressed.
