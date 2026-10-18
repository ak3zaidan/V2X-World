# Enhancing Radio Resource Management in RAN Slicing by Diffusion Model and Digital Twin
Shao-Wen Xiong, Yong-Ming Huang, Shi-Wen He, Cheng Zhang · IEEE Transactions on Communications 2025 · https://doi.org/10.1109/tcomm.2025.3594784 · relevance: low · basis: abstract

**Problem:** Network slicing is essential for the sixth-generation mobile networks.
**Approach:** Minimizing radio resources while guaranteeing service level agreement (SLA) remains challenging due to random traffic patterns and channel conditions, making policy security enforcement and underlying traffic distribution inference critical research goals. In this paper, to facilitate the design of policy agents for radio resource management, we first design a high-fidelity conditional generative diffusion model (CGDM)-driven digital twin network (DTN) to provide closed-loop interaction and pre-validation capabilities.
**Tools and scale:** Not stated in the abstract.
**Results:** Numerical results confirm that the built DTN exhibits good fidelity. Under fixed slices and stable traffic pattern, the DTN-enhanced approaches outperform the best baseline with an average SLA violation relative reduction of 71.3% and an average resource block utilization relative degradation of 10.9%, achieve about convergence speed enhancement of 80% compared to native CGDSAC, and is capable of adapting to the scenarios of dynamic number of slices and varying traffic patterns through knowledge transfer and pre-validation.
**Limitations and future work (as stated by the authors):**
- Minimizing radio resources while guaranteeing service level agreement (SLA) remains challenging due to random traffic patterns and channel conditions, making policy security enforcement and underlying traffic distribution inference critical research goals.
**Overlap with our engine:** partial — touches digital-twin modelling but not the joint traffic/radio/SCMS closed loop.
**Novelty implications:** Nothing in our candidate directions is ruled out; useful only as background on digital-twin modelling.
