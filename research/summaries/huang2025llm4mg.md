# LLM4MG: Adapting Large Language Model for Multipath Generation via Synesthesia of Machines
Ziwei Huang, Shiliang Lu, Lu Bai et al. · arXiv 2025 · https://arxiv.org/abs/2509.14711 · relevance: low · basis: abstract

**Problem:** Based on Synesthesia of Machines (SoM), a large language model (LLM) is adapted for multipath generation (LLM4MG) for the first time.
**Approach:** Based on the SynthSoM-V2I dataset, the proposed LLM4MG leverages Large Language Model Meta AI (LLaMA) 3.2 for multipath generation via multi-modal sensory data. The proposed LLM4MG aligns the multi-modal feature space with the LLaMA semantic space through feature extraction and fusion networks.
**Tools and scale:** Mentions: LiDAR.
**Results:** To further achieve general knowledge transfer from the pre-trained LLaMA for multipath generation via multi-modal sensory data, the low-rank adaptation (LoRA) parameter-efficient fine-tuning and propagation-aware prompt engineering are exploited. Simulation results demonstrate that the proposed LLM4MG outperforms conventional deep learning-based methods in terms of line-of-sight (LoS)/non-LoS (NLoS) classification with accuracy of 92.76%, multipath power/delay generation precision with normalized mean square error (NMSE) of 0.099/0.032, and cross-vehicular traffic density (VTD), cross-band, and cross-scenario generalization.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
