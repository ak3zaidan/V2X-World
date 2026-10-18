# SPADE: SPaT Attack Detection from the Connected Vehicle's Perspective
James Di Novo, Hany Ragab, Sylvain P. Leblanc · arXiv (Cornell University) 2026 · https://arxiv.org/abs/2609.02741 · relevance: high · basis: full text

**Problem:** No dataset for SPaT attack detection from the vehicle side.
**Approach:** SPADE dataset via runtime J2735 attack injection: six attack classes, four intersection geometries, six conditions, five seeds.
**Tools and scale:** Eclipse MOSAIC; 180 runs, about 1.89 million records; scenario-level split.
**Results:** Balanced dataset released with generation code.
**Limitations and future work (as stated by the authors):**
- None stated in the full text.
**Overlap with our engine:** strong — simulation-generated, seeded, labelled V2X attack benchmark with runtime injection (direction 3 benchmark side); no PKI or counterfactuals.
**Novelty implications:** Seeded sim-based attack datasets exist; per-unit counterfactual ground truth remains our claim.
