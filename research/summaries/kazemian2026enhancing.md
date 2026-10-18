# Enhancing 6G Wireless Intelligence: Do LLMs Work for CSI Prediction?
Mohsen Kazemian, Jürgen Jasperneite · arXiv 2026 · https://arxiv.org/abs/2604.04028 · relevance: low · basis: abstract

**Problem:** In high-mobility 6G scenarios, rapidly time-varying channels lead to very short coherence times, which makes conventional pilot-based channel state information (CSI) estimation approaches prone to outdated information or excessive pilot overhead.
**Approach:** This paper proposes an LLM-based framework for channel prediction in high-mobility orthogonal time frequency space (OTFS) communication systems. In this work, we develop a physics-aware LLM-based predictor that learns the temporal evolution of OTFS channel coefficients from historical channel observations while incorporating mobility-related physical descriptors (e.g., maximum Doppler frequency) to achieve accurate prediction of future channel states in rapidly time-varying environments.
**Tools and scale:** Mentions: deep learning.
**Results:** Numerical results show that the proposed method consistently achieves lower normalized mean square error (NMSE) compared with both classical deep learning predictors and LLM-based predictors without physical channel descriptors. These results demonstrate the advantage of integrating mobility-related channel knowledge with LLM-based sequence modeling for channel prediction in highly dynamic OTFS systems.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
