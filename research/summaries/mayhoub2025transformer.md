# Towards Transformer-Based Flow Volume Prediction in Network Traffic
Samara Mayhoub, Mirko Schiavone, C. Foh et al. · IEEE International Conference on High Performance Computing and Communications 2025 · https://doi.org/10.1109/hpcc67675.2025.00144 · relevance: low · basis: abstract

**Problem:** Accurately predicting network flow volume using early packet-level features is critical for real-time applications such as resource allocation, bottleneck prediction, anomaly detection and more.
**Approach:** Additionally, we train a lightweight Transformer model, and compare the predictive performance and complexity of all Transformer-based models to a simple Multi-Layer Perceptron (MLP) model. Results demonstrate that Transformer-based models significantly outperform the MLP model.
**Tools and scale:** Not stated in the abstract.
**Results:** The lightweight Transformer achieves the best $\mathrm{R}^{\mathrm{2}}$ score of 0.963 on CIC-IDS-2017, while NetFound achieves the best performance on the real-world traffic dataset MAWI with an $\mathrm{R}^{\mathrm{2}}$ of 0.812.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
