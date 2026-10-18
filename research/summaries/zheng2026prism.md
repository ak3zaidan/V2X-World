# PRISM-FL: Privacy-Preserving Robust Model Inspection via Scalar-Based Multi-Metric Verification for Vehicular Federated Learning
Yifeng Zheng, Qiang Nong, Wentao Zhou · preprint 2026 · https://doi.org/10.2139/ssrn.7285241 · relevance: low · basis: abstract

**Problem:** Federated learning (FL) enables collaborative intelligence in the Internet of Vehicles (IoV), yet defending against poisoned client model updates typically requires the aggregator to inspect update geometry, which conflicts with update confidentiality.
**Approach:** We propose PRISM-FL, a scalar-verification protocol for roadside-unit (RSU)-assisted vehicular federated learning (VFL) that resolves this tension through a controlled disclosure interface.
**Tools and scale:** Mentions: dataset (details not in abstract).
**Results:** On the VeReMi Extension dataset, PRISM-FL attains 64.20% accuracy under a 40% sign-flipping malicious-client ratio and remains stable across non-independent and identically distributed partitions; ablation shows that update-direction consistency is the dominant screening signal. Together, these results show that useful Byzantine-style screening evidence can be retained under a strictly reduced RSU observation model for VFL.
**Limitations and future work (as stated by the authors):**
- "These scalars support direction- and norm-based screening against sign-flipping, scaling, Gaussian-noise, and label-flipping model-update attacks, while the high-dimensional update remains behind a hybrid cryptographic path that combines lightweight client-side PASTA encryption with RSU-side PASTA-to-BFV transciphering and threshold BFV interfaces."
- "On the VeReMi Extension dataset, PRISM-FL attains 64.20% accuracy under a 40% sign-flipping malicious-client ratio and remains stable across non-independent and identically distributed partitions; ablation shows that update-direction consistency is the dominant screening signal."
**Overlap with our engine:** none — adjacent background; does not implement simulation, SCMS backend, or closed-loop evaluation comparable to the engine (judged from abstract).
**Novelty implications:** Rules out no candidate-direction claim on the available evidence; background reference only.
