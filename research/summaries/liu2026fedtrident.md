# FedTrident: Resilient Road Condition Classification Against Poisoning Attacks in Federated Learning
Sheng Liu, Panos Papadimitratos · arXiv 2026 · https://arxiv.org/abs/2603.19101 · relevance: medium · basis: abstract

**Problem:** FL has emerged as a transformative paradigm for ITS, notably camera-based Road Condition Classification (RCC).
**Approach:** To close this research gap, we propose FedTrident, which introduces: 1) neuron-wise analysis for local model misbehavior detection (notably including attack goal identification, critical feature extraction, and GMM-based model clustering and filtering); 2) adaptive client rating for client exclusion according to the local model detection results in each FL round; and 3) machine unlearning for corrupted global model remediation once malicious clients are excluded during FL.
**Tools and scale:** Not stated in the abstract.
**Results:** Extensive evaluation across diverse FL-RCC models, tasks, and configurations demonstrates that FedTrident can effectively thwart TLFAs, achieving performance comparable to that in attack-free scenarios and outperforming eight baseline countermeasures by 9.49% and 4.47% for the two most critical metrics.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses misbehaviour/attack detection (direction 1 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
