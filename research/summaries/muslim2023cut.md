# Cut-Out Scenario Generation With Reasonability Foreseeable Parameter Range From Real Highway Dataset for Autonomous Vehicle Assessment
H. Muslim, S. Endo, H. Imanaga, S. Kitajima et al. · IEEE Access 2023 · https://doi.org/10.1109/access.2023.3268703 · relevance: low · basis: abstract

**Problem:** This study aims to generate test cases for scenario-based assessment of automated driving systems (ADS) when encounter a cut-out maneuver where the lead vehicle having changed lanes, revealing a new lead vehicle that, in some cases, is slower than the original lead (the cutting-out) vehicle.
**Approach:** We extracted the cut-out scenarios from an established real-world traffic dataset recorded by instrumented vehicles on Japanese highways and then defined them using vehicle kinematic parameters (velocities and distances). The extracted scenarios were analyzed based on the direct correlation between every two consecutive vehicles: a rear part that describes the correlation between the following vehicle and the cutting-out vehicle; and a frontal part that describes the correlation between the cutting-out vehicle and the preceding vehicle.
**Tools and scale:** Mentions: dataset (details not in abstract).
**Results:** Results suggest that when the time headway between the following vehicle and the cutting-out vehicle is equal to or more than 2 s, there should not have collision risks between the following vehicle and the preceding vehicle.
**Limitations and future work (as stated by the authors):**
- None stated in the available text.
**Overlap with our engine:** none — adjacent background; does not implement simulation, SCMS backend, or closed-loop evaluation comparable to the engine (judged from abstract).
**Novelty implications:** Rules out no candidate-direction claim on the available evidence; background reference only.
