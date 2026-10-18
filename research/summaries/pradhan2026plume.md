# PLUME: Building a Network-Native Foundation Model for Wireless Traces via Protocol-Aware Tokenization
Swadhin Pradhan, Shazal Irshad, Jerome Henry · arXiv 2026 · https://arxiv.org/abs/2603.13647 · relevance: medium · basis: abstract

**Problem:** Wireless packet traces need protocol-native modelling.
**Approach:** Plume, 140M-parameter model on PDML dissections with protocol-aware tokenizer.
**Tools and scale:** 802.11 traces; five real failure categories; compared with frontier LLMs.
**Results:** 74-97% next-packet token accuracy; AUROC >= 0.99 zero-shot anomaly detection; 6.2x shorter sequences; >600x fewer parameters than frontier LLMs.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** partial — packet-trace foundation model; relevant to learning from our per-message logs.
**Novelty implications:** Trace foundation models exist for Wi-Fi; one over V2X with PKI ground truth remains open.
