# sumoITScontrol: Traffic Controller Collection for SUMO Traffic Simulations
Kevin Riehl, Anastasios Kouvelas, Michail A. Makridis · SUMO Conference Proceedings 2026 · https://doi.org/10.52825/scp.v7i.3284 · relevance: low · basis: abstract

**Problem:** Reliable benchmarking is essential for progress in intelligent traffic control research.
**Approach:** While microscopic traffic simulators such as SUMO enable detailed modelling of individual vehicle interactions, many published control studies still rely on single-run evaluations and project-specific baseline implementations, limiting reproducibility and comparability. This paper presents sumoITScontrol, an open-source and extensible Python framework providing a curated collection of widely used traffic controllers implemented for SUMO via the ...
**Tools and scale:** Vehicular setting. The available text does not describe a V2X simulator, radio stack, traffic model or SCMS-style PKI in enough detail to report; scale not stated.
**Results:** No quantitative results stated in the available text.
**Limitations and future work (as stated by the authors):**
- None stated in the available text.
**Overlap with our engine:** none: simulation/digital twins in a vehicular context. Low relevance background for the simulator itself; it does not model the joint traffic/radio/SCMS loop, closed-loop revocation or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the integrated V2X case (SCMS scale, channel load, closed-loop safety impact) is not addressed.
