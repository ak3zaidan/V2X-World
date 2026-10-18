# RA-LWLM: Retrieval-Augmented In-Context Localization with Wireless Foundation Models
Guangjin Pan, Hui Chen, Hei Victor Cheng, Henk Wymeersch · arXiv 2026 · https://arxiv.org/abs/2606.01899 · relevance: medium · basis: abstract

**Problem:** Wireless localization is a fundamental capability of sixth-generation (6G) networks. Conventional model-based methods require accurate modeling of the propagation environment and degrade in complex multipath and non-line-of-sight scenarios, while learning-based methods couple model parameters tightly to the training scene, requiring costly retraining ...
**Approach:** In this paper, we propose RA-LWLM, a retrieval-augmented in-context localization framework that achieves training-free cross-scene adaptation by externalizing scene-specific information into a per-scene fingerprint database rather than encoding it in model weights.
**Tools and scale:** Not stated in the abstract.
**Results:** Extensive ray-tracing-based experiments across heterogeneous scenes with diverse BS configurations show that RA-LWLM achieves nearly identical accuracy on seen and unseen scenes without any per-scene retraining, substantially outperforming end-to-end and FM-based baselines. These results validate the proposed retrieval-augmented in-context paradigm as a scalable solution for cross-scene localization in 6G networks.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — Learned wireless/PHY model (foundation or world model); no V2X security, PKI or closed-loop traffic.
**Novelty implications:** Shows learned radio surrogates are active work; a surrogate trained on joint traffic-radio-PKI ground truth with counterfactual labels is not addressed.
