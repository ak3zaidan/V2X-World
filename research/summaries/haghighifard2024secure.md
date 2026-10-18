# Secure Hierarchical Federated Learning in Vehicular Networks Using Dynamic Client Selection and Anomaly Detection
M. Saeid HaghighiFard, Sinem Coleri · arXiv 2024 · https://arxiv.org/abs/2405.17497 · relevance: medium · basis: abstract

**Problem:** Hierarchical Federated Learning (HFL) faces the significant challenge of adversarial or unreliable vehicles in vehicular networks, which can compromise the model's integrity through misleading updates.
**Approach:** Addressing this, our study introduces a novel framework that integrates dynamic vehicle selection and robust anomaly detection mechanisms, aiming to optimize participant selection and mitigate risks associated with malicious contributions. Through simulation-based performance evaluation, our proposed algorithm demonstrates remarkable resilience even under intense attack conditions.
**Tools and scale:** Through simulation-based performance evaluation, our proposed algorithm demonstrates remarkable resilience even under intense attack conditions.
**Results:** Our approach involves a comprehensive vehicle reliability assessment, considering historical accuracy, contribution frequency, and anomaly records. These anomaly records are then registered and combined with past performance for accuracy and contribution frequency to identify the most suitable vehicles for each learning round.
**Limitations and future work (as stated by the authors):**
- "Hierarchical Federated Learning (HFL) faces the significant challenge of adversarial or unreliable vehicles in vehicular networks, which can compromise the model's integrity through misleading updates."
**Overlap with our engine:** partial — addresses misbehaviour/attack detection (direction 1 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
