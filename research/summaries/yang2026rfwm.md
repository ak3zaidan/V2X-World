# RFWM: Physics-Guided World Model for Dynamic Wireless Radiance Field Generation
Zijiu Yang, Qianqian Yang · arXiv 2026 · https://arxiv.org/abs/2608.19709 · relevance: medium · basis: abstract

**Problem:** Radio-frequency (RF) radiance-field modeling is essential for wireless network optimization and sensing, yet remains challenging in dynamic and unseen environments. Existing learning-based methods synthesize RF fields from sparse measurements, but most struggle to generalize to dynamic and unseen environments.
**Approach:** To address this limitation, we propose RFWM, a physics-guided RF world model that maps multimodal physical conditions like visual dynamics and AP configurations to spatiotemporal RF fields.
**Tools and scale:** We construct a new benchmark of 7,715 sequences averaging 33 frames across 115 environments for dynamic RF-field generation.
**Results:** We construct a new benchmark of 7,715 sequences averaging 33 frames across 115 environments for dynamic RF-field generation. Experimental results show that RFWM improves MSE by approximately 7 dB and 3 dB over the state of the art under in-distribution and out-of-distribution settings, respectively.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — Learned wireless/PHY model (foundation or world model); no V2X security, PKI or closed-loop traffic.
**Novelty implications:** Shows learned radio surrogates are active work; a surrogate trained on joint traffic-radio-PKI ground truth with counterfactual labels is not addressed.
