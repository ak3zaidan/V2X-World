# How Evaluation Choices Change the Measured Benefit of Cooperative Perception: Evidence from Three V2X Benchmarks
Pincan Zhao, Yili Tang, Xinrui Zhang · arXiv 2026 · https://arxiv.org/abs/2610.01010 · relevance: medium · basis: abstract

**Problem:** Cooperative perception, in which connected vehicles and roadside infrastructure share sensor information, is a candidate enabler of automated mobility, and benchmark accuracy is the evidence cited when roadside deployment is considered.
**Approach:** This paper audits that evidence base across one simulated and two real-world vehicle-to-everything (V2X) benchmarks. In simulation, two widely studied robustness axes leave almost no recoverable headroom: an infrastructure-anchored pose correction returns about one accuracy point at every error level, and corrupting a partner costs 0.7 points.
**Tools and scale:** Mentions: real-world; scale not stated in the abstract.
**Results:** Cooperation is worth 7 to 15 accuracy points, yet 27% of frames on one split offer no partner and almost none do on another, so every regime-conditioned claim must name its split.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses V2X simulation/datasets (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
