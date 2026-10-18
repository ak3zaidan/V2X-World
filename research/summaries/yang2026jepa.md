# A JEPA-Based Field-Layer World Model for Bridging Channel Prediction and Estimation
Yuzhi Yang, Brahim Mefgouda, Hang Zou et al. · arXiv 2026 · https://arxiv.org/abs/2608.10222 · relevance: low · basis: abstract

**Problem:** Channel state information (CSI) acquisition, reconstruction, and prediction are fundamental yet costly tasks in modern MIMO-OFDM wireless systems.
**Approach:** Motivated by this observation, we propose a JEPA-based field-layer world model (FWM) that learns a shared latent propagation state from multi-resolution CSI observations across the considered carrier bands and predicts its task-relevant evolution in the latent domain. The proposed FWM maps multiple CSI observation resolutions to a shared latent propagation-field space through scale-specific tokenizer heads.
**Tools and scale:** Not stated in the abstract.
**Results:** Experiments on single-band and cross-band reconstruction demonstrate improved symbol detection and, more notably, substantial beamforming gains despite modest NMSE improvements, indicating that FWM captures task-relevant spatial propagation structure beyond coefficient-wise CSI fitting.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — Adjacent background (world model for wireless/mobile networks); does not touch the engine's core layers.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
