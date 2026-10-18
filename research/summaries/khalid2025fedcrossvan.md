# Fedcross‐VAN: Federated Cross‐Domain Behavior Alignment for VANET Intrusion Detection
Mudassar Khalid, Umer Zukaib, Mabrook S. Al-Rakhami et al. · Transactions on Emerging Telecommunications Technologies 2025 · https://doi.org/10.1002/ett.70309 · relevance: medium · basis: abstract

**Problem:** ABSTRACT Vehicular ad hoc networks (VANETs) enable the exchange of safety‐critical messages, but their open and dynamic nature makes them vulnerable to message falsification and denial‐of‐service attacks.
**Approach:** To address these limitations, we propose FedCross‐VAN, a novel FL framework that incorporates cross‐domain behavioral priors with a similarity‐weighted aggregation scheme .
**Tools and scale:** Not stated in the abstract.
**Results:** On the client side, FedCross‐VAN employs a dual‐objective loss that balances anomaly classification with alignment to external priors, improving generalization under heterogeneous data. Experiments on two benchmark datasets show that FedCross‐VAN achieves up to approximately 2%–4% higher accuracy on HAR and 13% on KWS, converges in fewer rounds, and exhibits stronger robustness than FedAvg and No‐Transfer FL.
**Limitations and future work (as stated by the authors):**
- "To address these limitations, we propose FedCross‐VAN, a novel FL framework that incorporates cross‐domain behavioral priors with a similarity‐weighted aggregation scheme ."
**Overlap with our engine:** partial — addresses misbehaviour/attack detection (direction 1 area); datasets/twins/surrogates (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
