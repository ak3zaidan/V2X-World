# Privacy Leakage in Federated Learning: Gradient-Based Client Identity Inference and Defenses for Inertial Sensing in Vehicular Edge Networks
Ali Akarma, Toqeer Ali Syed, Muhammad Khan, Qurat-ul-ain Mastoi et al. · arXiv 2026 · https://arxiv.org/abs/2609.02971 · relevance: low · basis: abstract

**Problem:** As vehicular networks move toward 5G/6G edge intelligence, federated learning (FL) is widely promoted as a privacy-preserving way for vehicles and infrastructure to train shared models without exposing raw sensor data.
**Approach:** We study server-side client identity inference from transmitted weight deltas using inertial (IMU) measurements, evaluated on the UCI Human Activity Recognition (HAR) benchmark as an accessible proxy for the IMU streams produced onboard connected vehicles.
**Tools and scale:** Mentions: 5g (details not in abstract).
**Results:** A practical region (sigma in [0.1, 0.2]) drives attack accuracy to near-random while costing under 5% relative FL accuracy. Results are supported by cryptographic (SHA-256) train/evaluation gradient disjointness, three seeds, and a count-normalized attacker-advantage metric.
**Limitations and future work (as stated by the authors):**
- None stated in the available text.
**Overlap with our engine:** none — adjacent background; does not implement simulation, SCMS backend, or closed-loop evaluation comparable to the engine (judged from abstract).
**Novelty implications:** Rules out no candidate-direction claim on the available evidence; background reference only.
