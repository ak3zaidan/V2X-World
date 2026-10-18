# Lightweight Adaptive Feature Composition for Heterogeneous Downstream Adaptation of Wireless Foundation Models
Yuxuan Shi, Tingting Yang, Li Sun, Liwen Jing et al. · arXiv 2026 · https://arxiv.org/abs/2606.10277 · relevance: medium · basis: abstract

**Problem:** Mobile systems increasingly rely on heterogeneous learning-enabled wireless functions, for which separate taskspecific models incur redundant training and model-management overhead. Wireless foundation models (WFMs) enable these functions to share a pretrained backbone, but existing adaptation either updates the backbone per task or relies on an inflexible ...
**Approach:** Based on this observation, we propose a Routing Adapter for Feature Composition (RAFC), which summarizes selected hidden states into compact descriptors, generates task- and sampledependent routing weights, and combines the original fullresolution features without updating the pretrained backbone.
**Tools and scale:** Experiments on a portable softwaredefined radio testbed further demonstrate improved end-to-end receiver robustness, particularly under partial-band co-channel interference.
**Results:** Experiments across four task categories and three WFM backbones show improvements in every evaluated backbone-task pair, with mean relative gains across the four task categories ranging from 2.9% to 35.9%, while RAFC adds only 0.003M-0.049M trainable parameters across the ... Experiments on a portable softwaredefined radio testbed further demonstrate improved end-to-end receiver robustness, particularly under partial-band co-channel interference.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — Learned wireless/PHY model (foundation or world model); no V2X security, PKI or closed-loop traffic.
**Novelty implications:** Shows learned radio surrogates are active work; a surrogate trained on joint traffic-radio-PKI ground truth with counterfactual labels is not addressed.
