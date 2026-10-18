# SplitLLM: Hierarchical Split Learning for Large Language Model over Wireless Network
Songge Zhang, Guoliang Cheng, Zuguang Li et al. · arXiv 2025 · https://arxiv.org/abs/2501.13318 · relevance: low · basis: abstract

**Problem:** Fine-tuning a large language model (LLM) using the local data of edge users can enable personalized services and applications.
**Approach:** In this paper, we propose a split learning (SL) scheme for fine-tuning LLM in wireless networks, which involves one cloud server, a small number of edge servers, and multiple users. Specifically, the pre-trained model and LoRA adapters are divided into three parts and deployed across the cloud, edge, and user sides.
**Tools and scale:** Not stated in the abstract.
**Results:** For privacy protection, the prevalent solution adopts distributed learning for fine-tuning and integrates low-rank adaptation (LoRA) to reduce users' computational load. The training process follows the sequence of user, edge, and cloud, with forward and backward propagation achieved by transmitting activation and gradient.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
