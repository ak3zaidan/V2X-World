# LatentWave: JEPA Pretraining for Wireless Foundation Models
Ahmed Mohamed, Ahmed Aboulfotouh, Hatem Abou-Zeid · arXiv 2026 · https://arxiv.org/abs/2606.06373 · relevance: medium · basis: abstract

**Problem:** Wireless foundation models have emerged as a promising alternative to building separate models for each wireless task. However, existing approaches rely on masked input reconstruction, which can bias representations toward low-level signal details.
**Approach:** In this paper, we propose LatentWave, a wireless foundation model pretrained using a Joint-Embedding Predictive Architecture (JEPA) on diverse wireless spectrograms and channel state information (CSI).
**Tools and scale:** Not stated in the abstract.
**Results:** We evaluate LatentWave on four downstream tasks: RF signal classification, 5G NR positioning, beam prediction, and LoS/NLoS classification, comparing against a masked-modeling baseline (WavesFM) pretrained on the same data. Additionally, we show that the masking geometry introduces a task-dependent inductive bias: frequency masking strongly favors channel-related tasks such as positioning and beam prediction, while region masking better preserves discriminability for signal classification.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — Learned wireless/PHY model (foundation or world model); no V2X security, PKI or closed-loop traffic.
**Novelty implications:** Shows learned radio surrogates are active work; a surrogate trained on joint traffic-radio-PKI ground truth with counterfactual labels is not addressed.
