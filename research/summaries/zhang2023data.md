# On Data Fabrication in Collaborative Vehicular Perception: Attacks and Countermeasures
Qing-Zhao Zhang, Shuo-Wei Jin, Ruiyang Zhu, Jiachen Sun, Xumiao Zhang, Q. Chen, Z. Morley Mao · USENIX Security Symposium 2023 · https://arxiv.org/abs/2309.12955 · relevance: medium · basis: abstract

**Problem:** Collaborative perception, which greatly enhances the sensing capability of connected and autonomous vehicles (CAVs) by incorporating data from external resources, also brings forth potential security risks. CAVs' driving decisions rely on remote untrusted data, making them susceptible to attacks carried out by malicious participants in the collaborative perception system.
**Approach:** To mitigate the vulnerability, we present a systematic anomaly detection approach that enables benign vehicles to jointly reveal malicious fabrication.
**Tools and scale:** Not stated in the abstract.
**Results:** Our attacks demonstrate a high success rate of over 86% on high-fidelity simulated scenarios and are realizable in real-world experiments. It detects 91.5% of attacks with a false positive rate of 3% in simulated scenarios and significantly mitigates attack impacts in real-world scenarios.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Trust/reputation management for detecting malicious senders; an alternative to the SCMS misbehaviour-report -> MA -> CRL pipeline our engine models, with no linkage-based revocation or time-to-neutralise analysis.
**Novelty implications:** Does not rule out any candidate direction; it does not model SCMS certificates, linkage-based revocation, per-message joint ground truth or counterfactual reruns.
