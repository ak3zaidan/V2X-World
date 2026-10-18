# Anomaly Detection in Intra-Vehicle Networks
Ajeet Kumar Dwivedi · arXiv 2022 · https://arxiv.org/abs/2205.03537 · relevance: medium · basis: abstract

**Problem:** The progression of innovation and technology and ease of inter-connectivity among networks has allowed us to evolve towards one of the promising areas, the Internet of Vehicles.
**Approach:** This paper discusses the security issues of the CAN bus protocol and proposes an Intrusion Detection System (IDS) that detects known attacks on in-vehicle networks. The main objective of this paper is to accurately detect cyberattacks by considering time-series features and attack frequency.
**Tools and scale:** Not stated in the abstract.
**Results:** The majority of the evaluated AI algorithms, when considering attack frequency, correctly identify known attacks with remarkable accuracy of more than 99%. However, these models achieve approximately 92% to 97% accuracy when timestamps are not taken into account.
**Limitations and future work (as stated by the authors):**
- "However, a primary challenge in the automotive industry is to make the vehicle safe and reliable; particularly with the loopholes in the existing traditional protocols, cyber-attacks on the vehicle network are rising drastically."
**Overlap with our engine:** partial — addresses misbehaviour/attack detection (direction 1 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
