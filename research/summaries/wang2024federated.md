# Federated Fine-Tuning for Pre-Trained Foundation Models Over Wireless Networks
Zixin Wang, Yong Zhou, Yuanming Shi, Khaled. B. Letaief · IEEE Transactions on Wireless Communications 2024 · https://doi.org/10.1109/twc.2025.3531128 · relevance: low · basis: abstract

**Problem:** Pre-trained foundation models (FMs), with extensive number of neurons, are key to advancing next-generation intelligence services, where personalizing these models requires massive amount of task-specific data and computational resources.
**Approach:** The prevalent solution involves centralized processing at the edge server, which, however, raises privacy concerns due to the transmission of raw data. Instead, federated fine-tuning (FedFT) is an emerging privacy-preserving fine-tuning (FT) paradigm for personalized pre-trained foundation models.
**Tools and scale:** Not stated in the abstract.
**Results:** In particular, by integrating low-rank adaptation (LoRA) with federated learning (FL), federated LoRA enables the collaborative FT of a global model with edge devices, achieving comparable learning performance to full FT while training fewer parameters over distributed data and preserving raw data privacy. Simulation results demonstrate the effectiveness of the proposed online algorithm in enhancing learning performance.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — touches network/traffic simulation but not the joint traffic/radio/SCMS closed loop.
**Novelty implications:** Nothing in our candidate directions is ruled out; useful only as background on network/traffic simulation.
