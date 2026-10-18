# DINO-CoDT: Multi-class Collaborative Detection and Tracking with Vision Foundation Models
Xunjie He, Christina Dao Wen Lee, Meiling Wang et al. · arXiv 2025 · https://arxiv.org/abs/2506.07375 · relevance: low · basis: abstract

**Problem:** Collaborative perception plays a crucial role in enhancing environmental understanding by expanding the perceptual range and improving robustness against sensor failures, which primarily involves collaborative 3D detection and tracking tasks.
**Approach:** To overcome these limitations, we propose a multi-class collaborative detection and tracking framework tailored for diverse road users. Next, we introduce a tracklet RE-IDentification (REID) module that leverages visual semantics with a vision foundation model to effectively reduce ID SWitch (IDSW) errors, in cases of erroneous mismatches involving small objects like pedestrians.
**Tools and scale:** Not stated in the abstract.
**Results:** Extensive experiments on the V2X-Real and OPV2V datasets show that our approach significantly outperforms existing state-of-the-art methods in both detection and tracking accuracy.
**Limitations and future work (as stated by the authors):**
- "This limitation hinders their applicability in real-world scenarios, which involve diverse object classes with varying appearances and motion patterns."
- "To overcome these limitations, we propose a multi-class collaborative detection and tracking framework tailored for diverse road users."
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
