# MultiPathFormer: Towards a Foundation Model for Multipath Wireless Propagation
Blessed Guda, Kayley Sze, Carlee Joe-Wong · arXiv 2026 · https://arxiv.org/abs/2608.05076 · relevance: medium · basis: abstract

**Problem:** Recent advances in machine learning have enabled training of wireless foundation models, which aim to support tasks such as channel estimation, beam prediction, and localization based on wireless signals. Existing wireless foundation models typically pretrain on channel tensors using masked reconstruction over subcarriers, antennas, or time but ignore the ...
**Approach:** In this work, we propose to instead use multipath propagation as the fundamental pretraining object.
**Tools and scale:** Not stated in the abstract.
**Results:** Across downstream tasks, it outperforms SOTA channel-based foundation models, achieving 5.57 m mean localization error, 0.914 top-3 beam accuracy, 0.994 line-of-sight classification accuracy, and 0.561 channel estimation NMSE. These results show that path-level pretraining can learn reusable representations of wireless propagation.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — Learned wireless/PHY model (foundation or world model); no V2X security, PKI or closed-loop traffic.
**Novelty implications:** Shows learned radio surrogates are active work; a surrogate trained on joint traffic-radio-PKI ground truth with counterfactual labels is not addressed.
