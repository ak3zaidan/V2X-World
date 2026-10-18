# UrbanV2X: A Multisensory Vehicle-Infrastructure Dataset for Cooperative Navigation in Urban Areas
Qijun Qin, Ziqi Zhang, Yihan Zhong, Feng Huang et al. · arXiv 2025 · https://doi.org/10.1109/itsc60802.2025.11423762 · relevance: high · basis: full text

**Problem:** Due to the limitations of a single autonomous vehicle, Cellular Vehicle-to-Everything (C-V2X) technology opens a new window for achieving fully autonomous driving through sensor information sharing. However, real-world datasets supporting vehicle-infrastructure cooperative navigation in complex urban environments remain rare.
**Approach:** To address this gap, we present UrbanV2X, a comprehensive multisensory dataset collected from vehicles and roadside infrastructure in the Hong Kong C-V2X testbed, designed to support research on smart mobility applications in dense urban areas. We also benchmark various navigation algorithms to evaluate the collected cooperative data.
**Tools and scale:** Real data from the Hong Kong C-V2X testbed: vehicle cameras, LiDARs, 4D radar, UWB, IMU, GNSS-RTK/INS; roadside LiDAR, GNSS, UWB; PTP time sync.
**Results:** Our onboard platform provides synchronized data from multiple industrial cameras, LiDARs, 4D radar, Ultra-wideband (UWB), IMU, and high-precision GNSS-RTK/INS navigation systems. The dataset is publicly available at: https://polyu-taslab.github.io/UrbanV2X/.
**Limitations and future work (as stated by the authors):**
- Authors plan to "enhance the richness of the data and the completeness of the evaluations".
- Collaborative SLAM methods not yet validated on the data.
- Annotations are still incomplete ("continue working on annotations").
**Overlap with our engine:** partial — Real V2X sensing dataset for cooperative navigation; no message-level radio, security or PKI ground truth, so it does not overlap our joint traffic/radio/PKI ground truth.
**Novelty implications:** We cannot claim to be the first urban C-V2X vehicle–infrastructure dataset; the security/PKI layer and counterfactual labels remain open.
