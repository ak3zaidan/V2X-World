# Research on Point Cloud Object Tampering Attacks for Cooperative Perception Systems
Yansong Shi, Siyu Chen, Hai Xiang et al. · 2025 IEEE Annual Congress on Artificial Intelligence of Things (AIoT) 2025 · https://doi.org/10.1109/aiot66900.2025.00146 · relevance: medium · basis: abstract

**Problem:** Cooperative perception (CP), as a vital component of vehicle-to-everything (V2X) technology, greatly enhances the perception capability of autonomous vehicles in complex traffic environments.
**Approach:** However, the reliance on shared external sensor data introduces critical security risks. This paper presents a novel point cloud object tampering attack framework targeting early-fusion CP systems, which enables both the injection of fake obstacles and the removal of real objects within shared LiDAR data.
**Tools and scale:** By leveraging the OpenCOOD framework and OPV2V dataset, we simulate these attacks in a collaborative perception setting and evaluate their impact on system performance.
**Results:** Experimental results demonstrate that the proposed method can significantly degrade the accuracy and reliability of perception outputs, effectively misleading detection models without triggering anomaly alarms.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses misbehaviour/attack detection (direction 1 area); datasets/twins/surrogates (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
