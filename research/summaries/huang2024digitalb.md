# Digital Twin Edge Services With Proximity-Aware Longitudinal Lane Changing Model for Connected Vehicles
Zeyin Huang, Rong Yu, Mingxing Ye, Songmiao Zheng, Jiawen Kang, Weiliang Zeng · IEEE Transactions on Vehicular Technology 2024 · https://doi.org/10.1109/tvt.2024.3412119 · relevance: medium · basis: abstract

**Problem:** Edge-assisted digital twin (DT) is an efficient way to monitor, analyze, and guide connected vehicles by creating a virtual representation of the vehicle at the edge.
**Approach:** This allows for real-time monitoring of vehicle performance, location, and condition, as well as providing guidance and edge services to optimize the vehicle's performance and ensure safe operation. Current traffic digital twinning methods primarily rely on data-driven approaches for predicting and replicating vehicle behavior, which do not adequately capture the complex dynamics and interactions between vehicles.
**Tools and scale:** Not stated in the abstract.
**Results:** More specifically, we first observe vehicle behavior, and find that the lane-changing behavior of the target vehicle is mainly influenced by neighboring vehicles, and ignoring the influence of distant vehicles can improve the accuracy of the PALLC model. In the experiments of different types of vehicles and different indicators, the results show that the PALLC model all have higher accuracy compared with the traditional method.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses V2X simulation/datasets (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
