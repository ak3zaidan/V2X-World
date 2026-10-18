# Deep Defence on Wheels: A Dual Intrusion Detection System Architecture for Comprehensive In-Vehicle Network Security
Shashwat Khandelwal, Shanker Shreejith · arXiv 2026 · https://arxiv.org/abs/2610.07489 · relevance: medium · basis: abstract

**Problem:** Increasing connectivity to the outside world and the lack of inbuilt security mechanisms have made legacy intra-vehicular networks vulnerable to cyberattacks.
**Approach:** We propose a dual IDS framework comprising supervised and unsupervised learning-based solutions, each optimised for real-time, resource-constrained automotive platforms. Both solutions are deployed and evaluated on the ZCU104 SoC (XCZU7EV FPGA), demonstrating a flexible hardware/software co-design for real-time detection of known and unknown attacks on high-speed CAN buses.
**Tools and scale:** Not stated in the abstract.
**Results:** Initial research focused on maximising detection accuracy for known and unknown attacks, often using large, full-precision machine learning models. Lightweight models must balance accuracy with these deployment constraints.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses misbehaviour/attack detection (direction 1 area); security overhead or cryptography (direction 2 area); datasets/twins/surrogates (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
