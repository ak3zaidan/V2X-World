# Scaling of Wireless Foundation Models via Representation Diversity and Multi-Branch Architectures
Ahmed Mohamed, Ahmed Aboulfotouh, Hatem Abou-Zeid · arXiv 2026 · https://arxiv.org/abs/2610.04289 · relevance: medium · basis: abstract

**Problem:** Wireless foundation models learn representations from unlabeled radio signals for reuse across downstream tasks. Scaling model capacity is a common strategy for learning richer representations and improving downstream performance.
**Approach:** We investigate objective diversity as an alternative scaling axis: different self-supervised objectives emphasize different signal properties, and combining their representations can preserve more information to enable diverse tasks.
**Tools and scale:** Not stated in the abstract.
**Results:** At an equal output embedding dimension, fusion outperforms the evaluated single-objective encoders on all six tasks while using roughly one-third of their parameters.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — Learned wireless/PHY model (foundation or world model); no V2X security, PKI or closed-loop traffic.
**Novelty implications:** Shows learned radio surrogates are active work; a surrogate trained on joint traffic-radio-PKI ground truth with counterfactual labels is not addressed.
