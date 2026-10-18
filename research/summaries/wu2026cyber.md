# Cyber Attack Detection Based on Finite-Time H ∞ Filter in Platoon-Based Connected Vehicle Systems
Zhong-Qiang Wu, Zi-Jian Zhang, Shi-Zhan Dong · IEEE Transactions on Vehicular Technology 2026 · https://doi.org/10.1109/tvt.2025.3616514 · relevance: medium · basis: abstract

**Problem:** Aiming at the issue of being susceptible to noise interference when detecting cyber-attacks in vehicle platooning, a detection method utilizing ellipsoid detector based on finite-time H∞filter is proposed.
**Approach:** Firstly, a finite-time H∞ filter is designed for vehicle longitudinal dynamics model with Unknown-ButBounded (UBB) noises, which can provide accurate state estimation for vehicles without depending on precise system model or statistical characteristics of the noise. Moreover, it ensures that the filter error remains within finite-time bounds and satisfies the specified H ∞ performance index to further improve the dynamic performance of the filter.
**Tools and scale:** Not stated in the abstract.
**Results:** Finally, the simulation results show that the designed detector has strong robustness to UBB noises and can effectively distinguish the cyber-attacks from external disturbances, and the detection sensitivity of small attack signal is better than that ofχ2detector with Kalman filter.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses misbehaviour/attack detection or trust (direction 1 area); V2X simulation/datasets (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
