# MORPH-U: Multi-Objective Resilient Motion Planning for V2X-Enabled Autonomous Driving in High-Uncertainty Environments via Simulation
Shih-Yu Lai · arXiv 2026 · https://arxiv.org/abs/2605.07370 · relevance: medium · basis: abstract

**Problem:** V2X can warn an autonomous vehicle about hazards beyond line-of-sight, but it also brings uncertainty: messages may be delayed, dropped, or even forged.
**Approach:** Meanwhile, map knowledge may change during a trip, forcing the vehicle to replan under tight real-time budgets. This paper studies how to make motion planning and low-level control robust to such uncertain, event-driven updates.
**Tools and scale:** Mentions: CARLA; scale not stated in the abstract.
**Results:** Experiments in dynamic CARLA scenarios show that V2X-augmented LDM improves downstream safety, Pareto tuning provides controllable accuracy-comfort trade-offs, and the gate prevents replanning under saturated false-DENM injection ($p_{\text{attack}}=1.0$).
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses V2X simulation/datasets (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
