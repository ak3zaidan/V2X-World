# Amplitude-Belief Reinforcement Learning for Adaptive Cyber Defense in Partially Observable V2X Networks
Anwar Shah, Rohan Farooq, Sajid Anwer, Tallha Akram, Usman Ghous, Sajid Ullah Khan · arXiv 2026 · https://arxiv.org/abs/2606.07796 · relevance: medium · basis: abstract

**Problem:** The Internet of Vehicles (IoV) creates a partially observable and adversarial V2X communication environment in which malicious vehicles may evade defensive mechanisms.
**Approach:** Existing IoV intrusion-detection methods provide limited support for sequential mitigation under adaptive attacker behavior. This paper formulates IoV cyber defense as a partially observable sequential decision problem and proposes Quantum Belief-Integrated Reinforcement Defense (Q-BIRD), an amplitude-belief reinforcement learning framework.
**Tools and scale:** Mentions: SUMO, Veins; scale not stated in the abstract.
**Results:** Q-BIRD reduces mean cumulative damage from 36.0 +- 5.5 to 28.0 +- 3.0 and damage variance from 12.0 +- 2.8 to 6.0 +- 1.5 compared with PPO using classical Bayesian belief. Communication-level results show that Q-BIRD maintains a packet delivery ratio of 0.94 +- 0.02, latency of 45 +- 6 ms, throughput of 3.60 +- 0.15 Mbps, and service availability of 0.95 +- 0.02.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses misbehaviour/attack detection or trust (direction 1 area); security overhead or cryptography (direction 2 area); V2X simulation/datasets (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
