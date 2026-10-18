# APEX: A Network-Native Time-Series Foundation Model for Forecasting and Anomaly Detection for Wireless Edge Operations
Swadhin Pradhan, Niloo Bahadori, Peiman Amini · arXiv 2026 · https://arxiv.org/abs/2606.11553 · relevance: low · basis: abstract

**Problem:** Generic time-series foundation models transfer poorly to wireless network telemetry whose signals are bursty, zero-inflated, and coupled across protocol layers.
**Approach:** We present APEX, a network-native, decoder-only transformer for forecasting enterprise AP telemetry, and evaluate it on DHCP degradation as a representative network task. On a 192-step (4-day) DHCP degradation benchmark, APEX-Large reduces MAE by 18% over the strongest foundation-model baseline (Toto) and 38% over SARIMA, with anomaly-detection F1 = 0.93, while APEX-Edge enables sub-second, privacy-preserving inference on AP-class edge hardware.
**Tools and scale:** Not stated in the abstract.
**Results:** These results suggest network-native pre-training is a practical foundation for proactive wireless operations.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
