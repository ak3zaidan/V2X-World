# VAST: V2X/Dynamic Map-Aware Autonomous Driving Systems Validation Toolchain
Shunsuke Ito, Takuya Azumi · arXiv 2026 · https://doi.org/10.23919/dsdcps00089.2026.00033 · relevance: low · basis: abstract

**Problem:** Cooperative autonomous driving in the IoT-to-Edge-to-Cloud continuum requires system-level validation across vehicles, infrastructure sensors, edge-side Dynamic Map services, and in-vehicle autonomous-driving stacks.
**Approach:** This paper presents VAST, a V2X/Dynamic Map-aware validation toolchain that connects Scenic, Scenario Simulator v2, AWSIM, Autoware, and SIM-LDM. VAST does not introduce a new search algorithm; instead, it addresses interoperability challenges, including Lanelet2-to-Scenic mapping, ROS 2-based co-simulation through SS2, Dynamic Map object injection into Autoware, and collection of TTC, PET, collision, timeout, and performance measurements.
**Tools and scale:** Vehicular setting. The available text does not describe a V2X simulator, radio stack, traffic model or SCMS-style PKI in enough detail to report; scale not stated.
**Results:** This paper presents VAST, a V2X/Dynamic Map-aware validation toolchain that connects Scenic, Scenario Simulator v2, AWSIM, Autoware, and SIM-LDM. In occluded-intersection scenarios, Lanelet2-compatible constrained sampling increases the edge-case discovery rate from 40.0% to 80.0% and reduces the average time per discovered edge case from 259.7 s to 110.4 s.
**Limitations and future work (as stated by the authors):**
- VAST does not introduce a new search algorithm; instead, it addresses interoperability challenges, including Lanelet2-to-Scenic mapping, ROS 2-based co-simulation through SS2, Dynamic Map object injection into Autoware, and collection of TTC, PET, collision, timeout, and performance measurements.
- A throughput study with 1-16 NPCs shows that sampling remains below 0.1 s, whereas AWSIM/Autoware execution and restart overhead dominate runtime.
**Overlap with our engine:** none: simulation/digital twins in a vehicular context. Low relevance background for the simulator itself; it does not model the joint traffic/radio/SCMS loop, closed-loop revocation or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the integrated V2X case (SCMS scale, channel load, closed-loop safety impact) is not addressed.
