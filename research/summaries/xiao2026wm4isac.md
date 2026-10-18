# WM4ISAC: World Model for Proactive ISAC Under Dynamic Blockage
Zichao Xiao, Linlong Wu, Bhavani Shankar Mysore R · arXiv 2026 · https://arxiv.org/abs/2610.08267 · relevance: low · basis: abstract

**Problem:** Dynamic blockage severely degrades vehicular integrated sensing and communication (ISAC), since narrow millimeter-wave beams must be adjusted before the line-of-sight (LoS) link is blocked or misaligned.
**Approach:** This paper proposes WM4ISAC, a proactive ISAC framework built on a world model that learns a reusable latent dynamics model of the traffic environment from downlink radar heatmaps, rather than directly mapping observations to beam indices. The proposed framework uses a vision (V) module to compress sensing observations, an action-conditioned memory (M) module to predict latent environment evolution, and lightweight event heads to evaluate LoS alignment, blockage, and scatter-assisted non-line-of-sight (NLoS) alignment for candidate beams.
**Tools and scale:** Not stated in the abstract.
**Results:** Simulations under dynamic blockage show that WM4ISAC improves the mean rate by $28.9\%$ over current LoS tracking, raises the 5\%-tile rate from $0.9$ Mbps to $30.5$ Mbps, and reaches $88.7\%$ of the perfect-information upper bound.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
