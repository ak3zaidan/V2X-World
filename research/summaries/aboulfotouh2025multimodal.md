# Multimodal Wireless Foundation Models
Ahmed Aboulfotouh, Hatem Abou-Zeid · ICC 2026 - IEEE International Conference on Communications 2025 · https://doi.org/10.1109/icc59461.2026.11587642 · relevance: medium · basis: abstract

**Problem:** Wireless foundation models (WFMs) have recently demonstrated promising capabilities, jointly performing multiple wireless functions and adapting effectively to new environments. However, while current WFMs process only one modality, depending on the task and operating conditions, the most informative modality changes and no single modality is best for all tasks.
**Approach:** In this work, we propose and build the first multimodal wireless foundation model capable of processing both raw IQ streams and image-like wireless modalities (e.g., spectrograms and CSI) and performing multiple tasks across both. We introduce masked wireless modeling for the multimodal setting, a self-supervised objective and pretraining recipe that learns a joint representation from IQ streams and image-like wireless modalities.
**Tools and scale:** Mentioned in abstract: 5G, 6G. (From the abstract only.)
**Results:** We evaluate the model on five tasks across both modality families: image-based (human activity sensing, RF signal classification, 5G NR positioning) and IQ-based (RF device fingerprinting, interference detection/classification). Our results demonstrates the strong potential of developing multimodal WFMs that support diverse wireless tasks across different modalities. We believe this provides a concrete step toward both AI-native 6G and the vision of joint sensing, communication, and localization.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** none — Adjacent background (foundation/pretrained model for wireless signals/channels); no direct overlap with the engine's traffic+radio+SCMS joint simulation.
**Novelty implications:** Does not restrict any candidate claim.
