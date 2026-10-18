# DT-DDNN: A Physical Layer Security Attack Detector in 5G RF Domain for CAVs
Ghazal Asemian, Mohammadreza Amini, Burak Kantarci et al. · arXiv 2024 · https://doi.org/10.1109/tvt.2025.3601032 · relevance: medium · basis: abstract

**Problem:** The Synchronization Signal Block (SSB) is a fundamental component of the 5G New Radio (NR) air interface, crucial for the initial access procedure of Connected and Automated Vehicles (CAVs), and serves several key purposes in the network's operation.
**Approach:** Leveraging RF domain knowledge, this work presents a novel deep learning-based technique for detecting jammers in CAV networks. Unlike the existing jamming detection algorithms that mostly rely on network parameters, we introduce a double-threshold deep learning jamming detector by focusing on the SSB.
**Tools and scale:** The Synchronization Signal Block (SSB) is a fundamental component of the 5G New Radio (NR) air interface, crucial for the initial access procedure of Connected and Automated Vehicles (CAVs), and serves several key purposes in the network's operation. These attacks, which can be executed without requiring high power or complex equipment, pose substantial risks to the 5G network, particularly as a r
**Results:** The detection method is focused on RF domain features and improves the robustness of the network without requiring integration with the pre-existing network infrastructure. Results show that the proposed method achieves 96.4% detection rate in extra low jamming power, i.e., SJNR between 15 to 30 dB.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses misbehaviour/attack detection (direction 1 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
