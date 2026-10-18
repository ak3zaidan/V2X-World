# Efficient Black-Box Adversarial Attacks for Deep Driving Maneuver Classification Models
Ankur Sarker, Haiying Shen, Tanmoy Sen, Quincy Mendelson · 2021 IEEE 18th International Conference on Mobile Ad Hoc and Smart Systems (MASS) 2021 · https://doi.org/10.1109/mass52906.2021.00072 · relevance: medium · basis: abstract

**Problem:** Deep Neural Network (DNN) models are expected to be widely used in self-driven autonomous vehicles to understand surrounding environments and enhance driving safety.
**Approach:** In this paper, we propose a Fast Black-box Adversarial (FBA) attack for time-series DNN models in connected autonomous vehicle (CAV) scenarios. In this attack, an attacker sends false driving signals to a vehicle to misclassify its DNN model (e.g., maintaining speed is misclassified to stopping).
**Tools and scale:** Not stated in the abstract.
**Results:** It significantly reduces the time to find the optimal perturbation since the initial solution is closer to the optimal solution. Our experiments based on real-driving datasets show the effectiveness of FBA in dealing with the two challenges compared with the existing black-box adversarial attacks.
**Limitations and future work (as stated by the authors):**
- In this paper, we propose a Fast Black-box Adversarial (FBA) attack for time-series DNN models in connected autonomous vehicle (CAV) scenarios.
- Though different black-box adversarial attacks have been proposed previously, they are mainly for image classification, which cannot be directly adopted in the CAV scenarios due to two challenges.
- To handle these two challenges, FBA consists of two steps for the adversarial signal generation: offline and online.
- Our experiments based on real-driving datasets show the effectiveness of FBA in dealing with the two challenges compared with the existing black-box adversarial attacks.
**Overlap with our engine:** partial — same area (V2X/vehicular security or networking) with a narrower, mechanism-level focus.
**Novelty implications:** Does not remove any of our claims beyond its specific mechanism; we cannot claim to be first on that mechanism. The joint closed-loop traffic/radio/PKI evaluation with counterfactual ground truth remains open as far as this abstract shows.
