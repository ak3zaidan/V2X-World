# MDRC: A Deployable State-Recovery Defense for Traffic Signal Control under Sensor Corruption
Mingyuan Li, Chunyu Liu, Xiao Liu, Yanna Jiang, Guangsheng Yu, Xu Wang et al. · arXiv 2026 · https://arxiv.org/abs/2609.27528 · relevance: medium · basis: abstract

**Problem:** Traffic Signal Control (TSC) is a safety-critical cyber-physical system that relies on real-time sensing.
**Approach:** Corrupted observations caused by adversarial perturbations or sensor failures can propagate from the sensing layer into the controller and degrade traffic efficiency. Existing robust Reinforcement Learning (RL)-based TSC methods often suffer from limited cross-city generalization, high inference latency, and weak recovery under partial observability.
**Tools and scale:** Mentions: real-world; scale not stated in the abstract.
**Results:** Across seven real-world-derived CityFlow benchmarks, MDRC reduces Average Travel Time by 6.77% under stochastic and policy-aware attacks and by 12.75% under structured sensor loss, while improving state-recovery fidelity. We further evaluate 3,600 seconds of real roadside measurements with 50% of detector channels disabled and integrate MDRC into a hardware-in-the-loop traffic-signal stack.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses misbehaviour/attack detection or trust (direction 1 area); V2X simulation/datasets (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
