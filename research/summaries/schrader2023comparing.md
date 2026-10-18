# Comparing Measured Driver Behavior Distributions to Results from Car-Following Models using SUMO and Real-World Vehicle Trajectories from Radar
Max Schrader, Mahdi Al Abdraboh, Joshua Bittle · SUMO Conference Proceedings 2023 · https://doi.org/10.52825/scp.v4i.214 · relevance: low · basis: abstract

**Problem:** In this study, the physical principles governing car-following (CF) behavior and their impact on traffic flow at signalized intersections are investigated.
**Approach:** High temporal-resolution radar data is used to provide valuable insights into actual CF behavior, including acceleration, deceleration, and time headway distribution. Demand-calibrated SUMO simulations are run using empirical CF parameter distributions, and three CF models are evaluated: IDM, EIDM, and Krauss.
**Tools and scale:** Vehicular setting. The available text does not describe a V2X simulator, radio stack, traffic model or SCMS-style PKI in enough detail to report; scale not stated.
**Results:** The findings reveal that measured accelerations differ from CF model parameter accelerations and using the empirical value ($\mu = 0.89m/s^2$) leads to unrealistic simulations that fail volume-based calibration.
**Limitations and future work (as stated by the authors):**
- Future work will investigate closing the loop between the measured real-world and SUMO distributions using traditional calibration tactics, as well as assess the impact of calibrated vs.
**Overlap with our engine:** none: simulation/digital twins in a vehicular context. Low relevance background for the simulator itself; it does not model the joint traffic/radio/SCMS loop, closed-loop revocation or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the integrated V2X case (SCMS scale, channel load, closed-loop safety impact) is not addressed.
