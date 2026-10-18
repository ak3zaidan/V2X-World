# Advancing CAN Network Security through RBM-Based Synthetic Attack Data Generation for Intrusion Detection Systems
Huacheng Li, Jingyong Su, Kai Wang · arXiv 2025 · https://arxiv.org/abs/2503.21496 · relevance: medium · basis: abstract

**Problem:** The rapid development of network technologies and industrial intelligence has augmented the connectivity and intelligence within the automotive industry.
**Approach:** To overcome this limitation, we introduce a novel methodology leveraging the Restricted Boltzmann Machine (RBM) to generate synthetic CAN attack data, thereby producing training datasets with a more balanced sample distribution. Specifically, we design a CAN Data Processing Module for transforming raw CAN data into an RBM-trainable format, and a Negative Sample Generation Module to generate data reflecting the distribution of CAN data frames denoting network intrusions.
**Tools and scale:** Not stated in the abstract.
**Results:** Experimental results show the generated data significantly improves IDS performance, with CANet accuracy rising from 0.6477 to 0.9725 and EfficientNet from 0.1067 to 0.1555.
**Limitations and future work (as stated by the authors):**
- "To overcome this limitation, we introduce a novel methodology leveraging the Restricted Boltzmann Machine (RBM) to generate synthetic CAN attack data, thereby producing training datasets with a more balanced sample distribution."
**Overlap with our engine:** partial — addresses misbehaviour/attack detection (direction 1 area); datasets/twins/surrogates (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
