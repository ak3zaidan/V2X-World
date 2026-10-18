# Kernel-Based Diffusion Model for O-RAN Digital Twins: Enabling Spatio-Temporally Consistent RSRP Generation
Bin Wu, Yuichi Ohsita, Nattaon Techasarntikul, Hideyuki Shimonishi · IEEE International Symposium on Personal, Indoor and Mobile Radio Communications 2026 · https://doi.org/10.1109/pimrc69400.2026.11711856 · relevance: medium · basis: abstract

**Problem:** Training and validating AI algorithms for the RAN Intelligent Controller (RIC) in O-RAN requires largescale and trustworthy data, yet such data are difficult to collect from operational networks. To address this challenge, we propose Kernel-Diff, a physics-informed diffusion model for O-RAN digital twins.
**Approach:** To address this challenge, we propose Kernel-Diff, a physics-informed diffusion model for O-RAN digital twins.
**Tools and scale:** Experiments on ns-3 5G-LENA data and real measurements collected on the Toyonaka campus of The University of Osaka show that Kernel-Diff achieves strong distribution fidelity and spatio-temporal consistency, while a lightweight adaptation pipeline effectively mitigates sim-to-real distribution ...
**Results:** By decomposing RSRP into a path loss baseline and a residual term, this model achieves better learning stability. Experiments on ns-3 5G-LENA data and real measurements collected on the Toyonaka campus of The University of Osaka show that Kernel-Diff achieves strong distribution fidelity and spatio-temporal consistency, while a lightweight adaptation pipeline effectively mitigates ...
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Network digital twin / learned network or traffic simulation; no V2X PKI or misbehaviour loop.
**Novelty implications:** Digital twins and learned simulators exist for other networks; joint traffic-radio-PKI counterfactual twin for V2X is not addressed.
