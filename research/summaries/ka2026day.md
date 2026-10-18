# Day-to-Day Traffic Network Modeling under Route-Guidance Misinformation: Endogenous Trust and Resilience in CAV Environments
Eunhan Ka, S. Ukkusuri · arXiv.org 2026 · https://arxiv.org/abs/2605.14204 · relevance: medium · basis: abstract

**Problem:** Connected and autonomous vehicles and smart mobility services increasingly use digital route guidance as an operational input to traffic network management.
**Approach:** When this information becomes unreliable or adversarial, day-to-day traffic models must represent not only flow adaptation but also the evolution of user trust in the information source. This paper develops a coupled day-to-day traffic assignment and trust-evolution framework for route-guidance misinformation.
**Tools and scale:** Not stated in the abstract.
**Results:** Numerical experiments on Sioux Falls, with an Anaheim robustness check, show that endogenous trust creates a threshold-based resilience mechanism. Above the threshold, trust erosion reduces the impact of the fixed-trust attack by about 91 percent in Sioux Falls and 85 percent in Anaheim.
**Limitations and future work (as stated by the authors):**
- "Below the trust-activation threshold, the attack remains behaviorally stealthy and dynamic trust provides almost no attenuation."
**Overlap with our engine:** partial — addresses misbehaviour/attack detection (direction 1 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
