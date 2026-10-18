# Anti-Byzantine attacks enabled vehicle selection for asynchronous federated learning in vehicular edge computing
Cui Zhang, Xiao Xu, Qiong Wu, Ping-Yi Fan et al. · China Communications 2024 · https://doi.org/10.23919/jcc.fa.2023-0718.202408 · relevance: medium · basis: abstract

**Problem:** In vehicle edge computing (VEC), asynchronous federated learning (AFL) is used, where the edge receives a local model and updates the global model, effectively reducing the global aggregation latency. Due to different amounts of local data, computing capabilities and locations of the vehicles, renewing the global model with same weight is inappropriate.
**Approach:** However, based on deep reinforcement learning (DRL), we can consider these factors comprehensively to eliminate vehicles with poor performance as much as possible and exclude vehicles that have suffered Byzantine attacks before AFL. At the same time, when aggregating AFL, we can focus on those vehicles with better performance to improve the accuracy and safety of the system.
**Tools and scale:** Car-Hacking/CAN dataset, federated learning (mentioned in abstract)
**Results:** Simulation results show that the proposed scheme effectively improves the safety and accuracy of the global model.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — In-vehicle/CAN or keyless-entry security; outside our V2X network/PKI scope.
**Novelty implications:** No effect on our novelty claims; background only.
