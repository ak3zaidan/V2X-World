# SiFo: Wireless Foundation Model for Low-Overhead Site-Specific CSI Feedback
Cheng-Jie Zhao, Zhaolin Wang, Zongyao Zhao, Yuanwei Liu · arXiv 2026 · https://arxiv.org/abs/2605.16141 · relevance: medium · basis: abstract

**Problem:** SiFo, a wireless foundation model-based framework, is proposed for low-overhead site-specific channel state information (CSI) feedback. In 3GPP NR, Type-II feedback provides an expressive codebook-based CSI representation, but it requires substantial reference-signal overhead, UE-side search, and feedback.
**Approach:** Learning-based site-specific feedback can reduce these online costs while retaining high-quality subspace representation by exploiting deployment-dependent propagation structure.
**Tools and scale:** Not stated in the abstract.
**Results:** Numerical results across ten city scenarios demonstrate that SiFo (i) achieves higher CSI-capture efficiency than separately trained site-specific learning baselines under the same target-site labeled budget, (ii) approaches the high-overhead 3GPP NR Type-II feedback reference ...
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — Learned wireless/PHY model (foundation or world model); no V2X security, PKI or closed-loop traffic.
**Novelty implications:** Shows learned radio surrogates are active work; a surrogate trained on joint traffic-radio-PKI ground truth with counterfactual labels is not addressed.
