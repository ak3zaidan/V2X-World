# Digital Twin of Wireless Physical Layer in Advanced Manufacturing Environment
Minhajur Rahman, John-Davis C. Oyedum, Ivan Seskar et al. · 2025 IEEE Future Networks World Forum (FNWF) 2025 · https://doi.org/10.1109/fnwf66845.2025.11317372 · relevance: low · basis: abstract

**Problem:** This paper presents a high-fidelity Digital Twin of the wireless physical layer for a representative manufacturing environment using Sionna’s ray-tracing engine.
**Approach:** A detailed 3D model of the factory floor is integrated with Sionna to generate path loss, channel impulse response (CIR), and link-level calculations for an OFDM system. CIR comparisons show an accurate capture of multipath components in most scenarios, with delay spread values that closely follow the measurements and remain within the bounds of the 3GPP factory model.
**Tools and scale:** Not stated in the abstract.
**Results:** The Digital Twin achieves a path loss exponent of 1.611 and shadow fading of 0.728 dB, closely matching the measured results and 3GPP Indoor Factory predictions, though underestimating shadow fading. BER(Bit Error Rate) results revealed a systematic offset, which we addressed by applying a gain calibration to the Digital Twin so that simulated and measured SNR values aligned, enabling consistent link-level predictions.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
