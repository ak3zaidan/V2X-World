# Machine Learning-Aided Latency Prediction in Packet-Switched Xhaul Networks
Mirosław Klinkowski, Jordi Perelló, Davide Careglio · IEEE Access 2026 · https://doi.org/10.1109/access.2026.3678383 · relevance: medium · basis: abstract

**Problem:** In this work, we address the challenge of accurately predicting latency in packet-switched Xhaul networks, enabling the convergent transport of fronthaul (FH) and midhaul (MH) traffic within radio access networks (RANs). Although deterministic worst-case (WC) models provide strict latency bounds, they tend to significantly overestimate actual flow ...
**Approach:** To address this limitation, we propose a machine learning-based (ML) latency prediction framework that leverages quantile regression (QR) to provide more accurate estimates of maximum one-way transmission latency for both FH and MH flows — an essential requirement for reliable RAN operation.
**Tools and scale:** Our approach enhances WC estimations by incorporating additional latency-related features and is validated using an extensive dataset generated from simulations of diverse ring and mesh topologies.
**Results:** A comparative analysis reveals that QR-based latency prediction outperforms WC latency estimations, significantly improving network performance by reducing the number of active DU processing nodes by up to 20% without compromising latency constraints. The results highlight the potential of ML techniques to enhance the accuracy of latency modeling in dynamic, latency-sensitive Xhaul scenarios, thereby contributing to the realization of RAN Digital Twin systems envisioned for future 6G networks.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Network digital twin / learned network or traffic simulation; no V2X PKI or misbehaviour loop.
**Novelty implications:** Digital twins and learned simulators exist for other networks; joint traffic-radio-PKI counterfactual twin for V2X is not addressed.
