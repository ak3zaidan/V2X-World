# SafeGraph-CAV: Trust-Weighted and Runtime-Shielded CACC for Cyber-Resilient Vehicle Platooning
Arawal Shukla, Revathi A.R. · Unknown venue 2026 · https://doi.org/10.2139/ssrn.7356338 · relevance: medium · basis: abstract

**Problem:** Vehicle-to-vehicle (V2V) communication enables cooperative adaptive cruise control (CACC), but it also exposes platoons to cyber-physical failure modes: spoofed, replayed, delayed, dropped, or collusive packets can enter the feedback loop as if they were valid measurements.
**Approach:** This work proposes SAFEGRAPH-CAV, a physics-guided trust-weighted CACC architecture with runtime shielding for cyber-resilient longitudinal platooning. SAFEGRAPH-CAV treats V2V packets as directed edge-level claims in a communication graph, estimates trust from relational and physics-residual features, injects trust weights into CACC, and applies a runtime shield when gap or time-tocollision constraints are threatened.
**Tools and scale:** Mentions: SUMO; scale not stated in the abstract.
**Results:** In held-out adversarial tests, SHIELDED SAFEGRAPH-CACC reduces RMS spacing error by 74.7% relative to raw CACC and 71.6% relative to chisquare CACC, while eliminating observed unsafe-gap violations, collisions, and crash penetration. In an empirical string-stability stress suite, it reduces RMS error by 88.2% relative to raw CACC and converts collision-prone behavior into empirically bounded, collision-free operation.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses misbehaviour/attack detection or trust (direction 1 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
