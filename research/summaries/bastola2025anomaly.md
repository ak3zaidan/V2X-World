# Anomaly Detection in Cooperative Vehicle Perception Systems Under Imperfect Communication
Ashish Bastola, Hao Wang, Abolfazl Razi · 2025 22nd International Conference on Ubiquitous Robots (UR) 2025 · https://doi.org/10.1109/ur65550.2025.11078073 · relevance: medium · basis: abstract

**Problem:** Anomaly detection in autonomous driving is crucial for safety and traffic efficiency.
**Approach:** Traditional single-vehicle systems face limitations due to restricted visibility and occlusions in complex scenarios. Cooperative Perception improves accuracy by sharing sensor data between vehicles, but challenges such as bandwidth constraints and unstable connections can lead to delayed or missing information.
**Tools and scale:** Not stated in the abstract.
**Results:** Cooperative Perception improves accuracy by sharing sensor data between vehicles, but challenges such as bandwidth constraints and unstable connections can lead to delayed or missing information. CPAD utilizes a graph-transformer architecture to model spatiotemporal correlation among vehicles and achieves a superior F1 gain of 15% and an AUC gain of 5% compared to conventional models.
**Limitations and future work (as stated by the authors):**
- Traditional single-vehicle systems face limitations due to restricted visibility and occlusions in complex scenarios.
- Cooperative Perception improves accuracy by sharing sensor data between vehicles, but challenges such as bandwidth constraints and unstable connections can lead to delayed or missing information.
- We also observe significant robustness against communication interruptions with an F1 gain of 19% under 25% sequential blackouts and 13% gain under 25% random blackouts compared to the best-performing model.
**Overlap with our engine:** partial — same area (V2X/vehicular security or networking) with a narrower, mechanism-level focus.
**Novelty implications:** Does not remove any of our claims beyond its specific mechanism; we cannot claim to be first on that mechanism. The joint closed-loop traffic/radio/PKI evaluation with counterfactual ground truth remains open as far as this abstract shows.
