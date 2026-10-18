# IPEK: Intelligent Priority-Aware Event-Based Trust with Asymmetric Knowledge for Resilient Vehicular Ad-Hoc Networks
İpek Abasıkeleş-Turgut · venue unknown 2026 · https://doi.org/10.2139/ssrn.6398699 · relevance: medium · basis: abstract

**Problem:** Vehicular Ad Hoc Networks (VANETs) are vulnerable to intelligent attackers who exploit the homogeneous treatment of traffic events in existing trust models.
**Approach:** This paper addresses this gap with IPEK. The proposed system rests on three mechanisms: an asymmetric local trust mechanism in which penalties scale with event and location severity while rewards follow an asymptotic model, making trust difficult to regain after misuse; a DST-based global trust fusion that adopts Yager’s combination rule—assigning conflicting evidence to uncertainty rather than forcing premature decisions— together with sequential source-reliability ordering and an asymmetric risk accentuation mechanism; and a strategic attack model, defined for evaluation, that behaves select
**Tools and scale:** Simulations using OMNeT++, Veins, and SUMO compare IPEK against two centralized, threshold-based schemes (MDT and TCEMD) via a detectionthreshold sweep.
**Results:** IPEK is the only method able to reduce its false positive rate to as low as 0.44%, whereas the baselines cannot go below roughly 22%. At matched false positive rates, IPEK attains the highest recall at every point (e.g., 0.781 at 5% FPR) and the highest F1-score (0.803) over a broad operating range.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses misbehaviour/attack detection (direction 1 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
