# Deep Real-Time Anomaly Detection for Connected Autonomous Vehicles
Rachid Oucheikh, Mouhsene Fri, Fayçal Fedouaki, M. Hain · EUSPN/ICTH 2020 · https://doi.org/10.1016/j.procs.2020.10.062 · relevance: medium · basis: abstract

**Problem:** Connected and autonomous vehicles (CAV) are expected to change the landscape of the automotive market. They are autonomous decision-making systems that process streams of observations coming from different external and on-board sensors.
**Approach:** In this paper, we propose a deep learning approach which consists of hierarchic models to firstly extract the signal features using an LSTM auto-encoder, then perform an accurate classification of each signal sequence in real-time. In addition, we investigated the impact of the model parameter tuning on the anomaly detection and the advantage of channel boosting through three scenarios.
**Tools and scale:** Not stated in the abstract.
**Results:** The model achieves an accuracy of 95.5% and precision of 94.2%.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — V2X/VANET attack or intrusion detection; overlaps our misbehaviour-detection component only at the detector level, without SCMS reporting, revocation or closed-loop safety measurement.
**Novelty implications:** Does not rule out any candidate direction; it does not model SCMS certificates, linkage-based revocation, per-message joint ground truth or counterfactual reruns.
