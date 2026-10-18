# Sequential and Generative Models for Vehicular Distributed MIMO Channel Prediction
Malek Abida, Rafik Zayani, Eric Simon, Davy Gaillot · arXiv 2026 · https://arxiv.org/abs/2606.25864 · relevance: medium · basis: abstract

**Problem:** Vehicular communication is a key 6G use case requiring reliable and high-capacity connectivity under fast mobility and highly time-varying propagation conditions. However, large-scale vehicular channel estimation is costly and limited, impacting system-level performance of vehicular communications, and realistic channel prediction models are needed.
**Approach:** This paper proposes a vehicular channel prediction framework based on real measured urban channels collected through a dedicated measurement campaign using the MaMIMOSA channel sounder.
**Tools and scale:** Not stated in the abstract.
**Results:** Results show that ChannelGPT achieves over 94% normalized mean squared error (NMSE) reduction compared to LSTM and significant improvements over other baselines, while reducing FLOPs by 28% and inference latency by 39% relative to the CNN + Transformer. Moreover, ChannelGPT-predicted channels yield SE distributions nearly indistinguishable from those obtained with real measurements, demonstrating its effectiveness for reliable performance evaluation in high-mobility 6G vehicular networks.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — Learned wireless/PHY model (foundation or world model); no V2X security, PKI or closed-loop traffic.
**Novelty implications:** Shows learned radio surrogates are active work; a surrogate trained on joint traffic-radio-PKI ground truth with counterfactual labels is not addressed.
