# A Context-aware Black-box Adversarial Attack for Deep Driving Maneuver Classification Models
Ankur Sarker, Haiying Shen, Tanmoy Sen · 2021 18th Annual IEEE International Conference on Sensing, Communication, and Networking (SECON) 2021 · https://doi.org/10.1109/secon52354.2021.9491584 · relevance: medium · basis: abstract

**Problem:** In a connected autonomous vehicle (CAV) scenario, each vehicle utilizes an onboard deep neural network (DNN) model to understand its received time-series driving signals (e.g., speed, brake status) from its nearby vehicles, and then takes necessary actions to increase traffic safety and roadway efficiency.
**Approach:** In the scenario, it is plausible that an attacker may launch an adversarial attack, in which the attacker adds unnoticeable perturbation to the actual driving signals to fool the DNN model inside a victim vehicle to output a misclassified class to cause traffic congestion and/or accidents. Such an attack must be generated in near real-time and the adversarial maneuver must be consistent with the current traffic context.
**Tools and scale:** Not stated in the abstract.
**Results:** Our extensive experimental studies using two different real driving datasets show that CBAA requires 43% fewer queries (to the DNN model to verify the attack success) and 53% less time than existing adversarial attacks.
**Limitations and future work (as stated by the authors):**
- However, previously proposed adversarial attacks fail to meet these requirements.
- To handle these challenges, in this paper, we propose a Context- aware Black-box Adversarial Attack (CBAA) for time-series DNN models in CAV scenarios.
**Overlap with our engine:** partial — same area (V2X/vehicular security or networking) with a narrower, mechanism-level focus.
**Novelty implications:** Does not remove any of our claims beyond its specific mechanism; we cannot claim to be first on that mechanism. The joint closed-loop traffic/radio/PKI evaluation with counterfactual ground truth remains open as far as this abstract shows.
