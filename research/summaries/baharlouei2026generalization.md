# Generalization of Supervised Federated Intrusion Detection in VANETs
Hamideh Baharlouei, Adetokunbo Makanju, Nur Zincir-Heywood · 2026 IEEE Intelligent Vehicles Symposium Workshops (IV Workshops) 2026 · https://doi.org/10.1109/ivworkshops71735.2026.11624032 · relevance: medium · basis: abstract

**Problem:** Detecting previously unseen attacks remains a key challenge in Vehicular Ad Hoc Networks (VANETs).
**Approach:** This paper investigates whether a supervised federated intrusion detection framework can implicitly support novelty detection without architectural modification.
**Tools and scale:** A leave-one-attack-out evaluation is conducted on the VeReMi-Extension dataset, where withheld attack variants differ in strategy but share distributed behavioral characteristics.
**Results:** The results show that the model maintains stable detection performance and low false positive rates on unseen DDoS variants. While this does not constitute full open-world novelty detection, it demonstrates that supervised detectors can exhibit practical robustness to previously unseen variants when attacks share similar traffic-level characteristics.
**Limitations and future work (as stated by the authors):**
- "Detecting previously unseen attacks remains a key challenge in Vehicular Ad Hoc Networks (VANETs)."
**Overlap with our engine:** partial — addresses misbehaviour/attack detection (direction 1 area); datasets/twins/surrogates (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
