# VehDyn: A Driving World Model Benchmark for Vehicle Dynamics
Tianyi Wang, Wangsheng Du, Jiazhou Chen, Tianyi Zeng, Xiangyu Li, Jiseop Byeon et al. · arXiv 2026 · https://arxiv.org/abs/2609.33264 · relevance: medium · basis: abstract

**Problem:** Video world models are emerging as data engines, action planners, and generative simulators for autonomous driving, but existing benchmarks primarily assess visual fidelity and coarse physical plausibility, providing limited evidence on whether generated driving futures obey realistic vehicle kinematics and dynamics.
**Approach:** This limitation is further compounded by the lack of datasets in which vehicle, road, maneuver, and speed conditions are independently controlled, and ground-truth vehicle states are recorded in synchrony with videos. We introduce VehDyn, a driving world model benchmark for vehicle dynamics.
**Tools and scale:** Mentions: CARLA, dataset; scale not stated in the abstract.
**Results:** DrivingWorld achieves the highest VehDyn score, followed by Cosmos 3 Nano and LTX-Video 2.5, and the VehDyn score agrees closely with human judgment.
**Limitations and future work (as stated by the authors):**
- This limitation is further compounded by the lack of datasets in which vehicle, road, maneuver, and speed conditions are independently controlled, and ground-truth vehicle states are recorded in synchrony with videos.
**Overlap with our engine:** partial — addresses V2X simulation/datasets (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
