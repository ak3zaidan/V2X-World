# Toward Resilient Vehicle Platooning: A Two-Layer Secure Control Architecture Against Hybrid Cyber-Physical Threats
Jian Gong, Lei Ding, Cheng-Feng Jia, Yutian Liu et al. · IEEE/CAA Journal of Automatica Sinica 2026 · https://doi.org/10.1109/jas.2026.125909 · relevance: low · basis: abstract

**Problem:** This paper presents a hierarchical secure control framework for resilient vehicular platooning under hybrid cyber-physical threats, including coupled false data injection (FDI) and denial-of-service (DoS) attacks, as well as actuator faults. A two-layer architecture is adopted to decouple cyber-layer disruptions from physical-layer execution, thereby enhancing system ...
**Approach:** At the upper layer, a virtual platoon system is constructed, where a distributed resilient controller integrated with an event-triggered mechanism (ETM) is developed to ensure coordinated behavior while reducing communication overhead. At the lower layer, an adaptive fault-tolerant tracking controller is designed to compensate for actuator degradation and external disturbances, enabling each physical vehicle to ...
**Tools and scale:** Vehicular setting (VANET/IoV/V2X). Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** Simulation results demonstrate that the proposed architecture effectively mitigates fault propagation, maintains robust performance under concurrent cyber and physical threats, and outperforms non-hierarchical benchmarks in terms of system stability.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial: vehicular security topic, but no simulation of traffic, radio, SCMS backend or safety outcomes is described.
**Novelty implications:** Rules out nothing specific in our candidate directions. It shows that lightweight V2X authentication protocols are widely proposed; we should not claim a new authentication scheme. Open: evaluation of such schemes in a closed-loop city-scale simulation with radio and safety ground truth.
