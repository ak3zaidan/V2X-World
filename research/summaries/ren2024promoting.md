# Promoting or Hindering: Stealthy Black-Box Attacks Against DRL-Based Traffic Signal Control
Yan Ren, Heng Zhang, Xianghui Cao, Chaoqun Yang et al. · IEEE Internet of Things Journal 2024 · https://doi.org/10.1109/jiot.2023.3308260 · relevance: high · basis: abstract

**Problem:** Numerous studies have demonstrated, in-depth, the vulnerability of the deep reinforcement learning (DRL) model’s elements (e.g., reward), which is a factor limiting the widespread deployment of DRL in some crucial domains, including intelligent traffic signal control (ITSC).
**Approach:** While partial poisoning attacks with insidious rewards are enabled undetectable by directly employing regularization or cumulative reward restrictions, these constraints are somewhat 1-D and fail to consider the time dependence of DRL. Moreover, the adversary should avoid injecting undesirable perturbations when agents’ policies are unstable, namely, effectively maximizing the attacking strategy’s benefit.
**Tools and scale:** Not stated in the available text unless given above (topic: traffic signal attacks).
**Results:** Extensive simulation results evaluated in SUMO, involving comparison assessment and attack distribution, exhibit a dramatic increase in average travel time, implying that our attacks impose pressure on the traffic flow, namely, the efficacy of proposed attack strategies.
**Limitations and future work (as stated by the authors):**
- None stated in the available text.
**Overlap with our engine:** partial — attacks on signal control; our engine simulates signals and in-sim attacks
**Novelty implications:** Does not affect novelty claims of the candidate directions beyond background in traffic signal attacks.
