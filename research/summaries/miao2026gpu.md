# GPU-Accelerated Real-Time SM2-Based Authentication in V2X Networks: Balancing Throughput and Latency Constraints
Zheng Miao, Qitao Feng, Hui Song, Shuai Wang et al. · venue not stated 2026 · https://doi.org/10.2139/ssrn.6428136 · relevance: low · basis: abstract

**Problem:** Vehicle-to-Everything (V2X) is a cornerstone of intelligent transportation and autonomous driving. It has strong requirements for high-speed and secure communication between vehicles and the environment.
**Approach:** In this work, we present a GPU-based SM2 elliptic-curve digital signature algorithm in the context of V2X intelligent driving. We also achieve controllable-latency and high-throughput signing for non-safety-critical scenarios.
**Tools and scale:** Vehicular setting (VANET/IoV/V2X). Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** However, compared to the traditional cryptographic algorithms, SM2 incurs significantly higher computational overhead in signature generation and verification. By employing thread-level fine-grained decomposition to optimize the core computational modules of the SM2 algorithm, the proposed method demonstrates remarkable performance improvements especially in safety-critical scenarios.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial: a V2X authentication/privacy scheme; our engine implements the standard SCMS/IEEE 1609.2 path rather than this protocol, and the paper reports no closed-loop traffic, radio or revocation simulation.
**Novelty implications:** Rules out nothing specific in our candidate directions. It shows that lightweight V2X authentication protocols are widely proposed; we should not claim a new authentication scheme. Open: evaluation of such schemes in a closed-loop city-scale simulation with radio and safety ground truth.
