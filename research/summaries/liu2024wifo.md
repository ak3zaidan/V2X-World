# WiFo: Wireless Foundation Model for Channel Prediction
Boxun Liu, Shijian Gao, Xuanyu Liu, Xiang Cheng et al. · arXiv 2024 · https://doi.org/10.1007/s11432-025-4349-0 · relevance: medium · basis: abstract

**Problem:** Channel prediction permits to acquire channel state information (CSI) without signaling overhead. However, almost all existing channel prediction methods necessitate the deployment of a dedicated model to accommodate a specific configuration.
**Approach:** Leveraging the powerful modeling and multi-task learning capabilities of foundation models, we propose the first space-time-frequency (STF) wireless foundation model (WiFo) to address time-frequency channel prediction tasks in a one-for-all manner. We propose a masked autoencoder (MAE)-based network structure for WiFo to handle heterogeneous STF CSI data, and design several mask reconstruction tasks for self-supervised pre-training to capture the inherent 3D variations of CSI.
**Tools and scale:** Not stated in the abstract. (From the abstract only.)
**Results:** To fully unleash its predictive power, we build a large-scale heterogeneous simulated CSI dataset consisting of 160K CSI samples for pre-training. Simulations validate its superior unified learning performance across multiple datasets and demonstrate its state-of-the-art (SOTA) zero-shot generalization performance via comparisons with other full-shot baselines.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** none — Adjacent background (foundation/pretrained model for wireless signals/channels); no direct overlap with the engine's traffic+radio+SCMS joint simulation.
**Novelty implications:** Does not restrict any candidate claim.
