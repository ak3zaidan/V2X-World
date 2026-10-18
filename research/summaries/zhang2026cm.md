# CM-MAE: A Physics-Guided Cross-Modal Self-Supervised Learning Framework for Vision-Wireless Applications
Yubo Zhang, Yi-Yao Liu · unknown venue 2026 · https://arxiv.org/abs/2608.15972 · relevance: low · basis: abstract

**Problem:** Synchronized camera and wireless measurements observe the same scene through different physical channels.
**Approach:** This paper presents CM-MAE, a self-supervised vision--wireless pretraining framework for cross-scenario representation transfer. The evaluated real-data model uses only RGB frames and the measured 64-beam received-power vector available in DeepSense 6G; it does not use ray-traced paths, calibrated depth, or beam-index labels during pretraining.
**Tools and scale:** Not stated in the abstract.
**Results:** Under a sequence-disjoint DeepSense 6G protocol, adding the soft alignment loss improves a matched linear-probe transfer average from 24.88\% to 29.49\%. Mild fusion fine-tuning reaches 77.38\% Top-1 accuracy on unseen Scenarios 6--8, and optional transductive normalization adaptation reaches 78.69\%.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — Adjacent background (foundation/pretrained model for wireless signals/channels); does not touch the engine's core layers.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
