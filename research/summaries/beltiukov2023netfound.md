# netFound: Principled Design for Network Foundation Models
Sylee Beltiukov, Satyandra Guthula, Haarika Manda, Jaber Daneshamooz, Wenbo Guo, Walter Willinger et al. · arXiv 2023 · https://arxiv.org/abs/2310.17025 · relevance: high · basis: full text (extracted sections)

**Problem:** Network foundation models exploit dataset shortcuts and miss exogenous conditions.
**Approach:** netFound: protocol-aware tokenisation, context embedding, burst-flow hierarchical attention, privacy-by-construction inputs; pretrained on billion-token corpus.
**Tools and scale:** 5000 GPU hours; packet traces; benchmarks incl. CIC-IDS-2017.
**Results:** F1 0.95 on exogenous context discrimination vs below 0.62 for prior models; top performer on all benchmarks.
**Limitations and future work (as stated by the authors):**
- Shortcut exploitation in common datasets (e.g., CIC-IDS-2017) must be fixed for valid evaluation.
**Overlap with our engine:** partial - network foundation model; not vehicular.
**Novelty implications:** Foundation models for network traffic exist; V2X foundation/world models over joint traces remain open.
