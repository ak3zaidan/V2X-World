# EfaSim - A Prototype for a SUMO-Driven Emergency Vehicle Driving Simulator
Rebecca Ahmed, Fabian Schuhmann, Cristian Cubides-Herrera, Markus Lienkamp · SUMO Conference Proceedings 2026 · https://doi.org/10.52825/scp.v7i.3323 · relevance: medium · basis: abstract

**Problem:** EfaSim (German: "Einsatzfahrt-Simulator", English: "Emergency Driving Simulator") is a prototype for an emergency vehicle driving simulator which uses the microscopic traffic simulation SUMO (Simulation of Urban Mobility) to simulate the behavior of other traffic participants.
**Approach:** Unreal Engine provides rendering power to visualize the environment, traffic, and the user interface. In the game engine, the user steers an emergency vehicle equipped with a toggleable blue light.
**Tools and scale:** Mentions: SUMO, Sumo; scale not stated in the abstract.
**Results:** Our goal is to create an efficient prototype that can simulate and render the game environment with a large number of simultaneously active vehicles while achieving an optimal frame rate of 60 frames per second (fps). The results show that both simulations operate on similar fps; however, the DLL achieves a measurable increase in information flow efficiency.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses V2X simulation/datasets (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
