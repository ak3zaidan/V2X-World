# A Pre-trained Large Model Based on Dual-Domain Compression for 6G Wireless Communication
Aili Zhang, Hao-Chen Li, Jian-Feng Wu et al. · International Conference on Smart City and Informatization 2025 · https://doi.org/10.1109/isci66495.2025.00013 · relevance: low · basis: abstract

**Problem:** For 6G communication scenarios in smart cities, this paper proposes a general air interface large model called WirelessChannelGPT based on the GPT-2 architecture, which can efficiently extract general knowledge representations of channels from high-dimensional raw wireless channel data in complex scenarios.
**Approach:** Firstly, a large-scale, multi-scenario (such as dense urban areas, transportation hubs, IoT sensing areas, etc.) and multi-distribution wireless channel dataset is used to conduct self-supervised training of a masked autoencoder for the general air interface large model, achieving efficient compression of channel information. Secondly, this paper explores a two-level separated network model architecture of "general large model + single-task small model".
**Tools and scale:** Not stated in the abstract.
**Results:** Dual-domain compression can enhance the large model's ability to understand the complex space-time-frequency characteristics of channels and improve the compression efficiency of the foundation model for channel features. Simulation experiments show that this scheme performs excellently in tasks such as channel estimation and channel prediction, providing strong support for the actual deployment and multi-task generalization of intelligent communication systems.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
