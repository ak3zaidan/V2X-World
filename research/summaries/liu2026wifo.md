# WiFo-MiSAC: A Wireless Foundation Model for Multimodal Sensing and Communication Integration via Synesthesia of Machines (SoM)
Xuanyu Liu, Shijian Gao, Boxun Liu, Xiang Cheng et al. · arXiv 2026 · https://arxiv.org/abs/2604.18255 · relevance: medium · basis: abstract

**Problem:** Current learning-based wireless methods struggle with generalization due to the fragmented processing of communication and sensing data. WiFo-MiSAC addresses this as a task-agnostic foundation model that tokenizes heterogeneous signals into a unified space for self-supervised pre-training.
**Approach:** A shared-specific disentangled mixture-of-experts (SS-DMoE) architecture is employed to decouple modality-shared and modality-specific representations, facilitating interaction without cross-modal interference.
**Tools and scale:** Not stated in the abstract. (From the abstract only.)
**Results:** By combining masked reconstruction with contrastive alignment, the model achieves state-of-the-art performance across downstream tasks, including beam prediction and channel estimation. Experimental results demonstrate robust few-shot adaptation and seamless integration of new modalities, positioning WiFo-MiSAC as a scalable backbone for future integrated sensing and communication systems.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** none — Adjacent background (foundation/pretrained model for wireless signals/channels); no direct overlap with the engine's traffic+radio+SCMS joint simulation.
**Novelty implications:** Does not restrict any candidate claim.
