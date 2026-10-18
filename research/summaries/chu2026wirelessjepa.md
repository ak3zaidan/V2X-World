# WirelessJEPA: A Multi-Antenna Foundation Model using Spatio-temporal Wireless Latent Predictions
Viet Chu, Omar Mashaal, Hatem Abou-Zeid · arXiv 2026 · https://arxiv.org/abs/2601.20190 · relevance: medium · basis: abstract

**Problem:** We propose WirelessJEPA, a novel wireless foundation model (WFM) that uses the Joint Embedding Predictive Architecture (JEPA). WirelessJEPA learns general-purpose representations directly from real-world multi-antenna IQ data by predicting latent representations of masked signal regions.
**Approach:** We propose WirelessJEPA, a novel wireless foundation model (WFM) that uses the Joint Embedding Predictive Architecture (JEPA).
**Tools and scale:** WirelessJEPA learns general-purpose representations directly from real-world multi-antenna IQ data by predicting latent representations of masked signal regions.
**Results:** To adapt JEPA to wireless signals, we introduce a 2D antenna time representation that reshapes multi-antenna IQ streams into structured grids, allowing convolutional processing with block masking and efficient sparse computation over unmasked patches. We evaluate WirelessJEPA across six downstream tasks and demonstrate it's robust performance and strong task generalization.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — Learned wireless/PHY model (foundation or world model); no V2X security, PKI or closed-loop traffic.
**Novelty implications:** Shows learned radio surrogates are active work; a surrogate trained on joint traffic-radio-PKI ground truth with counterfactual labels is not addressed.
