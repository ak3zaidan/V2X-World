# Enhanced Secure CAN Framework with SHA-256 Authentication and Intrusion Detection for Connected Vehicles
Vamsi Krishna A, Priti S. Pavale · International Conference on Artificial Intelligence and Machine Learning Applications Theme: Healthcare and Internet of Things (AIMLA) 2026 · https://doi.org/10.1109/aimla67915.2026.11522546 · relevance: medium · basis: abstract

**Problem:** Modern automobiles consist of multiple electronic modules that exchange operational data through internal vehicular communication networks.
**Approach:** Despite its reliability and real-time communication capabilities, the CAN protocol was originally designed without built-in security mechanisms. This work presents a lightweight security architecture for CAN communication that integrates SHA-256-based message authentication with a gateway-level intrusion detection mechanism.
**Tools and scale:** A three-node architecture consisting of Sender ECU, Gateway ECU, and Receiver ECU is modeled and evaluated using MATLAB/Simulink.
**Results:** Experimental analysis under both normal and adversarial scenarios demonstrates that the proposed framework effectively mitigates replay and injection attacks while maintaining minimal computational overhead.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses misbehaviour/attack detection (direction 1 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
