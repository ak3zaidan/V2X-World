# A Suspicion-Free Black-box Adversarial Attack for Deep Driving Maneuver Classification Models
Ankur Sarker, Haiying Shen, Tanmoy Sen · 2021 IEEE 41st International Conference on Distributed Computing Systems (ICDCS) 2021 · https://doi.org/10.1109/icdcs51616.2021.00080 · relevance: medium · basis: abstract

**Problem:** The current autonomous vehicles are equipped with onboard deep neural network (DNN) models to process the data from different sensor and communication units.
**Approach:** In the connected autonomous vehicle (CAV) scenario, each vehicle receives time-series driving signals (e.g., speed, brake status) from nearby vehicles through the wireless communication technologies. In the CAV scenario, several black-box adversarial attacks have been proposed, in which an attacker deliberately sends false driving signals to its nearby vehicle to fool its onboard DNN model and cause unwanted traffic incidents.
**Tools and scale:** Not stated in the abstract.
**Results:** We devise a gradient estimation technique for the knowledge gradient method to reduce the solution searching time. The experimental results show that SBBA outperforms the previous adversarial attacks by 56% higher success rate under detection methods, 238% less time to launch the attacks, and 76% less perturbation (to avoid being detected), and 257% fewer queries (to the DNN model to verify the attack success).
**Limitations and future work (as stated by the authors):**
- In the CAV scenario, several black-box adversarial attacks have been proposed, in which an attacker deliberately sends false driving signals to its nearby vehicle to fool its onboard DNN model and cause unwanted traffic incidents.
- However, the previously proposed black-box adversarial attack can be easily detected.
- To handle this problem, in this paper, we propose a Suspicion-free Boundary Black-box Adversarial (SBBA) attack, where the attacker utilizes the DNN model's output to design the adversarial perturbation.
**Overlap with our engine:** partial — same area (V2X/vehicular security or networking) with a narrower, mechanism-level focus.
**Novelty implications:** Does not remove any of our claims beyond its specific mechanism; we cannot claim to be first on that mechanism. The joint closed-loop traffic/radio/PKI evaluation with counterfactual ground truth remains open as far as this abstract shows.
