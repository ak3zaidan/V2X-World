# Safe and Secure Control of Connected and Automated Vehicles: An Event-Triggered Control Approach Using Trust-Aware Robust Control Barrier Functions
H. M. Sabbir Ahmad, Ehsan Sabouni, Wei Xiao et al. · ACM Transactions on Cyber-Physical Systems 2025 · https://doi.org/10.1145/3776545 · relevance: medium · basis: abstract

**Problem:** We address the security of a network of Connected and Automated Vehicles (CAVs) cooperating to safely navigate through a conflict area (e.g., traffic intersections, merging roadways, roundabouts).
**Approach:** To combat these, motivated by recent work on the safe control of CAVs, we propose a trust-aware robust event-triggered decentralized control and coordination framework that can provably guarantee safety. Moreover, we show that our proposed trust framework is immune to false positives.
**Tools and scale:** We use extensive simulations in SUMO and CARLA to validate the theoretical guarantees and demonstrate the efficacy of our proposed scheme to detect and mitigate adversarial attacks. The code for the simulated scenarios are available at https://github.com/SabbirAhmad26/Trust_based_CBF .
**Results:** We use extensive simulations in SUMO and CARLA to validate the theoretical guarantees and demonstrate the efficacy of our proposed scheme to detect and mitigate adversarial attacks.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses misbehaviour/attack detection (direction 1 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
