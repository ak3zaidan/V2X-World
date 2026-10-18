# Uncertainty Quantification for Collaborative Object Detection Under Adversarial Attacks
Huiqun Huang, Cong Chen, Jean-Philippe Monteuuis, Jonathan Petit, Fei Miao · arXiv 2025 · https://arxiv.org/abs/2502.02537 · relevance: medium · basis: abstract

**Problem:** Collaborative Object Detection (COD) and collaborative perception can integrate data or features from various entities, and improve object detection accuracy compared with individual perception.
**Approach:** However, adversarial attacks pose a potential threat to the deep learning COD models, and introduce high output uncertainty. With unknown attack models, it becomes even more challenging to improve COD resiliency and quantify the output uncertainty for highly dynamic perception scenes such as autonomous vehicles.
**Tools and scale:** Mentions: dataset; scale not stated in the abstract.
**Results:** We evaluate TUQCP on V2X-Sim, a comprehensive collaborative perception dataset for autonomous driving, and demonstrate a 80.41% improvement in object detection accuracy compared to the baselines under the same adversarial attacks.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses misbehaviour/attack detection or trust (direction 1 area); V2X simulation/datasets (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
