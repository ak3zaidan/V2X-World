# Protocol-Aware Tokenization and Architecture Co-Design for Wireless Packet Foundation Models
Swadhin Pradhan, Shazal Irshad, Jerome Henry · arXiv 2026 · https://arxiv.org/abs/2606.20587 · relevance: medium · basis: abstract

**Problem:** What matters more for building foundation models for wireless packet traces: the tokenizer or the architecture or both? To answer this question, we build on PLUME Anonymous [2026], which introduced protocol-aware tokenization for 802.11 traces; we scale model depth and transfer the same tokenizer to a fundamentally different architecture family.
**Approach:** To answer this question, we build on PLUME Anonymous [2026], which introduced protocol-aware tokenization for 802.11 traces; we scale model depth and transfer the same tokenizer to a fundamentally different architecture family.
**Tools and scale:** Not stated in the abstract.
**Results:** A deeper GPT (PLUME-DEEP, 24 layers) reaches 98.2% top-1 accuracy, gaining 32 points over the original 12-layer design, while a Mamba-2 state-space variant (PLUME-MAMBA) achieves 96.1% with 1.7x higher throughput and 2x longer context. The key insight emerges from a controlled 2x2 comparison across tokenizers and architectures: changing the tokenizer swings accuracy by 32 points; changing the architecture moves it by only 2.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — Learned wireless/PHY model (foundation or world model); no V2X security, PKI or closed-loop traffic.
**Novelty implications:** Shows learned radio surrogates are active work; a surrogate trained on joint traffic-radio-PKI ground truth with counterfactual labels is not addressed.
