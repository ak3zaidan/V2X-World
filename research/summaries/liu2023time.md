# Time-Series Misalignment Aware DNN Adversarial Attacks for Connected Autonomous Vehicles
Qichang Liu, Haiying Shen, Tanmoy Sen, Rui Ning · 2023 IEEE 20th International Conference on Mobile Ad Hoc and Smart Systems (MASS) 2023 · https://doi.org/10.1109/mass58611.2023.00039 · relevance: medium · basis: abstract

**Problem:** Connected Autonomous Vehicle (CAV) technique allows each autonomous vehicle to analyze time-series driving signals (e.g., speed, steering wheel angle) sent by its nearby vehicle using the Deep Neural Network (DNN).
**Approach:** This scenario could attract adversarial attacks on the DNNs. To fool the victim's DNN model to make misclassification in the black-box scenario, an attacker generates a small perturbation series on time-series signals to generate an adversarial input.
**Tools and scale:** Not stated in the abstract.
**Results:** Both of our experimental evaluations based on real driving data and real experiments show that MA3 produces up to 63.49% higher success rate, 56% less perturbation generation time, and 63.16% less perturbation amount compared with previous attack method.
**Limitations and future work (as stated by the authors):**
- To fool the victim's DNN model to make misclassification in the black-box scenario, an attacker generates a small perturbation series on time-series signals to generate an adversarial input.
- However, as the attacker does not know the start time stamp of each input to the DNN, the start point of the perturbation may not match that of real target input and the misalignment can make the attack unsuccessful.
**Overlap with our engine:** partial — same area (V2X/vehicular security or networking) with a narrower, mechanism-level focus.
**Novelty implications:** Does not remove any of our claims beyond its specific mechanism; we cannot claim to be first on that mechanism. The joint closed-loop traffic/radio/PKI evaluation with counterfactual ground truth remains open as far as this abstract shows.
