# TruPercept: Trust Modelling for Autonomous Vehicle Cooperative Perception from Synthetic Data
Braden Hurl, Robin Cohen, Krzysztof Czarnecki, Steven Waslander · 2020 IEEE Intelligent Vehicles Symposium (IV) 2020 · https://doi.org/10.1109/iv47402.2020.9304695 · relevance: high · basis: full text

**Problem:** Cooperative perception assumes received object detections are correct; malicious or unreliable peers break that assumption.
**Approach:** TruPercept: trust modelling of peers based on local verification of their reported 3D detections, trust-weighted fusion, plus a synthetic multi-vehicle perception dataset with unreliable and malicious scenarios and a modular evaluation framework.
**Tools and scale:** Synthetic urban driving world (game-engine based) producing multi-vehicle LiDAR/camera data; latency and alignment set aside; no radio or PKI model.
**Results:** Up to 5% Average Precision increase from trust-tempered fusion.
**Limitations and future work (as stated by the authors):**
- Latency and alignment issues set aside
- future: role-based trust (e.g. police cars), incentives for honest reporting, clustering of objects to reduce information, central server reputation, Bayesian/POMDP trust models.
**Overlap with our engine:** partial — misbehaviour in cooperative perception with ground truth; no network, PKI or revocation.
**Novelty implications:** Trust-weighted cooperative perception with a malicious-scenario dataset exists; no closed loop to SCMS.
