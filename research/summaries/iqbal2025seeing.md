# Seeing the City Live: Bridging Edge Vehicle Perception and Cloud Digital Twins to Empower Smart Cities
Hafsa Iqbal, Jaime Godoy, Beatriz Martin, Abdulla Al-kaff, Fernando Garcia · Smart Cities 2025 · https://doi.org/10.3390/smartcities8060197 · relevance: medium · basis: abstract

**Problem:** This paper presents a framework that integrates real-time onboard (ego vehicle) perception module with edge processing capabilities and a cloud-based digital twin for intelligent transportation systems (ITSs) in smart city applications.
**Approach:** The proposed system combines onboard 3D object detection and tracking with low latency edge-to-cloud communication, achieving an average end-to-end latency below 0.02 s at 10 Hz update frequency. Experiments conducted on a real autonomous vehicle platform demonstrate a mean Average Precision (mAP@40) of 83.5% for the 3D perception module.
**Tools and scale:** Not stated in the abstract.
**Results:** The proposed system enables real-time traffic visualization while enabling scalable data management by reducing communication overhead.
**Limitations and future work (as stated by the authors):**
- Future work will extend the system to multi-vehicle deployments and incorporate additional environmental semantics such as traffic signal states, road conditions, and predictive Artificial Intelligence (AI) models to enhance decision support in dynamic urban environments.
**Overlap with our engine:** partial — addresses security overhead or cryptography (direction 2 area); V2X simulation/datasets (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
