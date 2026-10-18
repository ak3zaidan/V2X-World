# Enhanced Deep Reinforcement Learning for Robustness Falsification of Partially Observable Cyber-Physical Systems
Yangwei Xing, Ting Shu, Xuesong Yin, Jinsong Xia · Symmetry 2026 · https://doi.org/10.3390/sym18020304 · relevance: medium · basis: abstract

**Problem:** Robustness falsification is a critical verification task for ensuring the safety of cyber-physical systems (CPS).
**Approach:** Under partially observable conditions, where internal states are hidden and only input–output data is accessible, existing deep reinforcement learning (DRL) approaches for CPS robustness falsification face two key limitations: inadequate temporal modeling due to unidirectional network architectures, and sparse reward signals that impede efficient exploration. These limitations severely undermine the efficacy of DRL in black-box falsification, leading to low success rates and high computational costs.
**Tools and scale:** Not stated in the abstract.
**Results:** Results show that DRL-BiT-MPR increases the falsification success rate by an average of 39.6% compared to baseline methods and reduces the number of simulations by more than 50.2%.
**Limitations and future work (as stated by the authors):**
- Under partially observable conditions, where internal states are hidden and only input–output data is accessible, existing deep reinforcement learning (DRL) approaches for CPS robustness falsification face two key limitations: inadequate temporal modeling due to unidirectional network architectures, and sparse reward signals that impede efficient exploration.
- These limitations severely undermine the efficacy of DRL in black-box falsification, leading to low success rates and high computational costs.
- This study addresses these limitations by proposing DRL-BiT-MPR, a novel framework whose core innovation is the synergistic integration of a bidirectional temporal network with a multi-granularity reward function.
**Overlap with our engine:** partial — addresses V2X simulation/datasets (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
