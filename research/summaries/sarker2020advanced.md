# An Advanced Black-Box Adversarial Attack for Deep Driving Maneuver Classification Models
Ankur Sarker, Haiying Shen, Tanmoy Sen, Hua Uehara · 2020 IEEE 17th International Conference on Mobile Ad Hoc and Sensor Systems (MASS) 2020 · https://doi.org/10.1109/mass50613.2020.00032 · relevance: medium · basis: abstract

**Problem:** Connected and autonomous vehicles (CAV) have been introduced to increase roadway safety and traffic flow efficiency.
**Approach:** In CAV scenarios, an autonomous vehicle shares its current and near-future driving maneuvers in terms of different driving signals (e.g., speed, brake pedal pressure) with its nearby vehicles using wireless communication technologies. Deep neural network (DNN) models are usually used to process the driving maneuver time-series data over other machine learning algorithms due to the high prediction accuracy of DNN models.
**Tools and scale:** Not stated in the abstract.
**Results:** We first formulate an optimization problem for the attack generation with continuous search space to reduce the search time. To solve the optimization problem, A3 innovatively combines the binary search and optimization algorithm to improve the time-efficiency of searching the optimal solution.
**Limitations and future work (as stated by the authors):**
- In CAV scenarios, an autonomous vehicle shares its current and near-future driving maneuvers in terms of different driving signals (e.g., speed, brake pedal pressure) with its nearby vehicles using wireless communication technologies.
- The existing black-box adversarial attacks (which are for image datasets) require many queries to the DNN model to check if a generated attack will be successful (hence long time) or high amount of perturbation (low imperceptibility), and thus cannot be applied to the time-sensitive CAV scenarios fe
- In this paper, we present an Advanced black-box Adversarial Attack $({\mathrm {A}}^{3})$ for the deep driving maneuver classification models.
- The experimental results show that the A3 attack requires up to 84.12% fewer queries and 57.67% less perturbation with 94.87% higher success rates than the existing black-box adversarial attacks.
**Overlap with our engine:** partial — same area (V2X/vehicular security or networking) with a narrower, mechanism-level focus.
**Novelty implications:** Does not remove any of our claims beyond its specific mechanism; we cannot claim to be first on that mechanism. The joint closed-loop traffic/radio/PKI evaluation with counterfactual ground truth remains open as far as this abstract shows.
