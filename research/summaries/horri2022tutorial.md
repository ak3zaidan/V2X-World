# A Tutorial and Review on Flight Control Co-Simulation Using Matlab/Simulink and Flight Simulators
Nadjim Horri, Mikolaj Pietraszko · Automation 2022 · https://doi.org/10.3390/automation3030025 · relevance: medium · basis: abstract

**Problem:** Flight testing in a realistic three-dimensional virtual environment is increasingly being considered a safe and cost-effective way of evaluating aircraft models and their control systems.
**Approach:** The paper starts by reviewing and comparing the most popular personal computer-based flight simulators that have been successfully interfaced to date with the MathWorks software. This co-simulation approach allows combining the strengths of Matlab toolboxes for functions including navigation, control, and sensor modeling with the advanced simulation and scene rendering capabilities of dedicated flight simulation software.
**Tools and scale:** Mentions: Matlab; scale not stated in the abstract.
**Results:** For a smaller ZOHD Nano Talon miniature aerial vehicle (MAV), Simulink is interfaced with the VFTE, which was specifically designed for this MAV, and with QGroundControl for the testing of advanced H-infinity observer-based autopilots using a software-in-the-loop (SIL) simulation to achieve robust low altitude flight under windy conditions.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses V2X simulation/datasets (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
