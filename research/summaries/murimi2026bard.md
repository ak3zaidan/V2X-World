# BARD-MARL: Byzantine-Agent Detection for Learned Communication in Multi-Agent Reinforcement Learning
Almond Kiruthu Murimi · arXiv 2026 · https://arxiv.org/abs/2606.20701 · relevance: medium · basis: abstract

**Problem:** Learned communication improves coordination in cooperative multi-agent reinforcement learning, but it also creates a trust problem: a trained policy may route information through agents that have become faulty or adversarial.
**Approach:** This paper studies Byzantine-agent detection for learned-communication MARL in adaptive traffic signal control. We propose BARD-MARL, a post-hoc diagnostic layer on top of BayesG, which is used as an attributed communication substrate rather than as a contribution of this paper.
**Tools and scale:** Mentions: SUMO; scale not stated in the abstract.
**Results:** Across fixed-action, observation-flip, random-noise, and coordinated attacks in SUMO traffic grids, the results show that these signals are complementary rather than universally dominant. On a 25-agent grid, BARD-MARL reaches 0.843 AUC-ROC under a 10% observation-flip attack, while policy-graph-only detection reaches 0.917 AUC-ROC under a 10% coordinated attack.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses misbehaviour/attack detection or trust (direction 1 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
