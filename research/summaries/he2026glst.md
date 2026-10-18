# GLST: Defending Confidence-Driven V2X Collaborative Perception Against Stealthy Multi-Attacker Feature Injection
Ji He, Ying Wang, Lijie Zheng et al. · arXiv 2026 · https://arxiv.org/abs/2607.23059 · relevance: medium · basis: abstract

**Problem:** Collaborative perception (CP) improves autonomous-driving perception by enabling connected vehicles to exchange intermediate features via V2X.
**Approach:** Using Where2comm as a representative framework, we show that the proposed Pretend Benign attack exploits its spatial-confidence mechanism by injecting stealthy perturbations into uncertain yet perception-critical regions, substantially degrading 3D object detection while preserving benign-like feature characteristics. We therefore propose Global-Local Structural Trust (GLST), a lightweight defense that assesses collaborator reliability through three complementary perspectives: global feature consistency, multi-scale local residual consistency, and structural consistency with ego-side semantic 
**Tools and scale:** Not stated in the abstract.
**Results:** Collaborative perception (CP) improves autonomous-driving perception by enabling connected vehicles to exchange intermediate features via V2X. Confidence-driven sparse communication reduces bandwidth by transmitting only perception-critical spatial regions, but creates a security risk: once a collaborator is compromised, malicious features in high-confidence or ego-uncertain regions may be preferentially selected and amplified during fusion.
**Limitations and future work (as stated by the authors):**
- "GLST also remains effective against gradient-based attacks such as PGD, indicating that multi-level trust modeling is essential for securing confidence-driven CP."
**Overlap with our engine:** partial — addresses misbehaviour/attack detection (direction 1 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
