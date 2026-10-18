# Simulation based method for the analysis of energy-efficient driving algorithms using SUMO
Benedikt Buhk, Rasmus Rettig · SUMO Conference Proceedings 2026 · https://doi.org/10.52825/scp.v2i.91 · relevance: low · basis: abstract

**Problem:** The limited possibilities to evaluate the energy efficiency of driving algorithms for connected and autonomous vehicles (CAVs) make it very difficult for policymakers to decide on the potential of autonomous driving.
**Approach:** This study is introducing a method to analyze the energy performance of a driving algorithm under various simulated traffic conditions using the microscopic traffic simulator SUMO. The method can also be used to optimize driving algorithm parameters for chosen traffic scenarios.
**Tools and scale:** Vehicular setting. The available text does not describe a V2X simulator, radio stack, traffic model or SCMS-style PKI in enough detail to report; scale not stated.
**Results:** To validate the model, three measurement cycles under real urban traffic conditions were taken and the speed and state of charge (SOC) data of the test vehicle, a 2017 Tesla Model S 75D, were collected.
**Limitations and future work (as stated by the authors):**
- Also, the significance of auxiliary system use on the energy consumption and its effect on a driving algorithm’s parameter to remain energy efficient due to the higher impact of the trip duration is pointed out.
**Overlap with our engine:** none: simulation/digital twins in a vehicular context. Low relevance background for the simulator itself; it does not model the joint traffic/radio/SCMS loop, closed-loop revocation or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the integrated V2X case (SCMS scale, channel load, closed-loop safety impact) is not addressed.
