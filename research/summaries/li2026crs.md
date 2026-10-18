# CRS-LLM: Cooperative Beam Prediction with a GPT-Style Backbone and Switch-Gated Fusion
Fangzhi Li, Cunhua Pan, Hong Ren et al. · arXiv 2026 · https://arxiv.org/abs/2604.27945 · relevance: low · basis: abstract

**Problem:** Millimeter-wave (mmWave) communication depends on highly directional beamforming, while fast mobility, blockage, and rapid geometry changes in vehicle-to-everything (V2X) scenarios make beam tracking challenging.
**Approach:** To address this issue, this paper proposes Cooperative Radio Sensing with Large Language Models (CRS-LLM), a cooperative beam prediction framework for next-step joint BS-beam prediction.
**Tools and scale:** Mentions: CNN.
**Results:** Simulation results show that CRS-LLM outperforms CSI-Transformer, Hierarchical BS-Beam, and representative CNN- and recurrent-neural-network baselines in Top-1 accuracy and normalized beam gain under different SNR conditions, while also showing strong few-shot performance and promising zero-shot transferability.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
