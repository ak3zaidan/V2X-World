# Mixed Platoon Control under Noise and Attacks: Robust Data-Driven Predictive Control and Human-in-the-Loop Validation
Shuai Li, Chaoyi Chen, Haotian Zheng et al. · arXiv 2024 · https://arxiv.org/abs/2411.13924 · relevance: medium · basis: abstract

**Problem:** Controlling mixed platoons, which consist of both connected and automated vehicles (CAVs) and human-driven vehicles (HDVs), poses significant challenges due to the uncertain and unknown human driving behaviors.
**Approach:** To address this issue, this paper proposes a Robust Data-EnablEd Predictive Leading Cruise Control (RDeeP-LCC) framework based on data-driven reachability analysis. The framework over-approximates system dynamics under noise and attack using a matrix zonotope set derived from data, and develops a stabilizing feedback control law.
**Tools and scale:** Not stated in the abstract.
**Results:** Human-in-the-loop experiments demonstrate that the RDeeP-LCC method significantly improves robustness against noise and attacks, while enhancing tracking accuracy, control efficiency, energy economy, driving comfort, and driving safety.
**Limitations and future work (as stated by the authors):**
- "Controlling mixed platoons, which consist of both connected and automated vehicles (CAVs) and human-driven vehicles (HDVs), poses significant challenges due to the uncertain and unknown human driving behaviors."
**Overlap with our engine:** partial — addresses misbehaviour/attack detection (direction 1 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
