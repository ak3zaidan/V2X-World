# A Universal Adversarial Attack on Time-Series DNNs for Connected Autonomous Vehicles
Qichang Liu, Tanmoy Sen, Haiying Shen et al. · 2022 IEEE 19th International Conference on Mobile Ad Hoc and Smart Systems (MASS) 2022 · https://doi.org/10.1109/mass56207.2022.00050 · relevance: medium · basis: abstract

**Problem:** In the Connected Autonomous Vehicle (CAV) scenarios, each autonomous vehicle uses the Deep Neural Network (DNN) to process time-series driving signals (e.g., speed, steering wheel angle) sent by its nearby vehicle to output its driving maneuver.
**Approach:** To further improve the time-efficiency performance, in this paper, we propose a method that generates one universal offline perturbation irrespective of the contexts in producing an untargeted adversarial attack. The experiments show that the proposed universal perturbation method requires 52.8% smaller number of queries, 30.7% less perturbation amount and 42.02% less time than other existing universal methods while keeping a high success rate of 87.09%.
**Tools and scale:** We evaluate the universal perturbation method on real driving dataset.
**Results:** No quantitative results stated in the abstract.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses misbehaviour/attack detection (direction 1 area); datasets/twins/surrogates (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
