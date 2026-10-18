# Distributed Trajectory Optimization for Connected and Automated Vehicle Platoons Considering Safe Inter-Vehicle Following Gaps
Meiqi Liu, Ying Gao, Yikai Zeng, Ruochen Hao · Systems 2025 · https://doi.org/10.3390/systems13060483 · relevance: medium · basis: abstract

**Problem:** Existing studies on platoon trajectory optimization of connected and automated vehicles face challenges in balancing computational efficiency, privacy, and safety.
**Approach:** This study proposes a distributed optimization method that decomposes the platoon trajectory planning problem into independent individual vehicle tasks while ensuring safe inter-vehicle following gaps and maximizing travel efficiencyand ride comfort. The individual vehicle problems independently optimize their trajectory to improve computational efficiency, and only exchange dual variables related to safe following gaps to preserve privacy.
**Tools and scale:** Not stated in the abstract.
**Results:** Simulation results demonstrate that the mean computation time is reduced by 50% and the fuel consumption is decreased by 4% compared to the centralized method while effectively maintaining the safe inter-vehicle following gaps.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses pseudonym privacy/linkability (runner-up area); V2X simulation/datasets (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
