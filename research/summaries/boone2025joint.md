# A Joint Reconstruction-Triplet Loss Autoencoder Approach Towards Unseen Attack Detection in IoV Networks
Julia Boone, Tolunay Seyfi, Fatemeh Afghah · arXiv 2025 · https://arxiv.org/abs/2505.21703 · relevance: medium · basis: abstract

**Problem:** Internet of Vehicles (IoV) systems, while offering significant advancements in transportation efficiency and safety, introduce substantial security vulnerabilities due to their highly interconnected nature.
**Approach:** These dynamic systems produce massive amounts of data between vehicles, infrastructure, and cloud services and present a highly distributed framework with a wide attack surface. Here, we present an unsupervised autoencoder method trained entirely on benign network data for the purpose of unseen attack detection in IoV networks.
**Tools and scale:** Not stated in the abstract.
**Results:** We show that our method performs robustly for all unseen attack types, with roughly 99% accuracy on benign data and between 97% and 100% performance on anomaly data. We extend these results to show that our model is adaptable through the use of transfer learning, achieving similarly high results while leveraging domain features from one domain to another.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses misbehaviour/attack detection (direction 1 area); datasets/twins/surrogates (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
