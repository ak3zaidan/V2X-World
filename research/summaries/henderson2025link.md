# Link-to-System Mapping for 5G NR Sidelink System Simulations
Thomas R. Henderson, Peng Liu, Chunmei Liu, Fernando J. Cintrón · 2025 IEEE World Forum on Public Safety Technology (WF-PST) 2025 · https://doi.org/10.1109/wf-pst65083.2025.00023 · relevance: low · basis: abstract

**Problem:** We describe new packet error models for the Network Simulator version 3 (ns-3) New Radio (NR) sidelink simulation models, derived from an extensive simulation campaign using the National Institute of Standards and Technology (NIST) Fifth Generation (5G) NR Sidelink Link-Level Simulator (LLS).
**Approach:** The new error models support the system-level simulation of future 5G sidelink-based public safety scenarios. We first describe the scope, methodology, and output of the link-level campaign and its validation.
**Tools and scale:** Vehicular setting. The available text does not describe a V2X simulator, radio stack, traffic model or SCMS-style PKI in enough detail to report; scale not stated.
**Results:** We next describe the options available to map this data into ns-3 error models, and discuss how we handled various integration issues including interpolation, extrapolation, the need for per-retransmission error tables in ns-3, and how to handle retransmissions with varying signal-to-noise ratio (SNR).
**Limitations and future work (as stated by the authors):**
- The new error models support the system-level simulation of future 5G sidelink-based public safety scenarios.
- Finally, we illustrate the sensitivity of the models to changes in coding, frequency band, numerology, retransmission attempts, and transport block size, and describe our future plans.
**Overlap with our engine:** none: simulation/digital twins in a vehicular context. Low relevance background for the simulator itself; it does not model the joint traffic/radio/SCMS loop, closed-loop revocation or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the integrated V2X case (SCMS scale, channel load, closed-loop safety impact) is not addressed.
