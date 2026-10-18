# Adaptive Transit Signal Priority Based on Deep Reinforcement Learning and Connected Vehicles in a Traffic Micro Simulation Environment
Dickness Kakitahi Kwesiga, Angshuman Guin, Michael Hunter · 2024 Winter Simulation Conference (WSC) 2024 · https://doi.org/10.1109/wsc63780.2024.10838807 · relevance: medium · basis: abstract

**Problem:** Model-free reinforcement learning (RL) provides a potential alternative to earlier formulations of adaptive transit signal priority (TSP) algorithms based on mathematical programming that require complex and nonlinear objective functions.
**Approach:** This study extends RL-based traffic control to include TSP. Using a micro-scopic simulation environment and connected vehicle data, the study develops and tests a TSP event-based RL agent that assumes control from another developed RL-based general traffic signal controller.
**Tools and scale:** Not stated in the abstract.
**Results:** This agent is shown to reduce the bus travel time by about 21 %, with marginal impact to general traffic at a saturation rate of 0.95. The architecture of the agent and simulation is selected considering the need to improve simulation run time efficiency.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses V2X simulation/datasets (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
