# Extending SUMO for Lane-Free Microscopic Simulation of Connected and Automated Vehicles
Dimitrios Troullinos, Georgios Chalkiadakis, Diamantis Manolis, Ioannis Papamichail, Markos Papageorgiou · SUMO Conference Proceedings 2022 · https://doi.org/10.52825/scp.v3i.110 · relevance: medium · basis: abstract

**Problem:** This paper presents some new developments related to TrafficFluid-Sim, a lane-free microscopic simulator that extends the SUMO simulation infrastructure to model lane-free traffic environments, allowing vehicles to be located at any lateral position, disregarding standard notions of car-following and lane-change maneuvers that are typically embedded within a (lanebased) simulator.
**Approach:** A dynamic library has been designed for traffic monitoring and lane-free vehicle movement control, one that does not impose any inter-tool “communication” delays that standard practices with the TraCI module introduce; and enables the emulation of vehicleto-vehicle and vehicle-to-infrastructure communication. We first summarize the various core components that constitute our simulator, and then discuss the new capability to utilize the bicycle kinematic model, additionally to the usual double-integrator model, as a more realistic model of vehicle movement dynamics, particularly for a lane-free traffic environment.
**Tools and scale:** Mentions: SUMO; scale not stated in the abstract.
**Results:** No quantitative results stated in the abstract.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses V2X simulation/datasets (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
