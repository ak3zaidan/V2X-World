# RAN-JEPA: An Action-Conditioned World Model for Energy-efficient Mobile Network Control
Binghan Wu, Kai Zeng, Kai Yang, Ya-Qin Zhang et al. · arXiv 2026 · https://arxiv.org/abs/2610.17584 · relevance: medium · basis: abstract

**Problem:** Operational mobile networks are rich in observations but poor in interventions: they continuously generate large volumes of passive telemetry, while controlled action--response trajectories remain comparatively scarce and costly to collect. This observation--intervention asymmetry is a central obstacle to building practical world models for RAN control.
**Approach:** We evaluate RAN-JEPA using public operator performance data for pretraining and a real-equipment four-cell 5G testbed for domain adaptation, post-training, and control.
**Tools and scale:** Not stated in the abstract.
**Results:** In closed-loop control, RAN-JEPA consistently supports higher sleep ratios than the two baselines at comparable congestion cost. These results show that observation-first, action-conditioned learning can turn abundant passive telemetry and limited intervention data into a practical predictive model for energy-aware RAN control.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — Learned wireless/PHY model (foundation or world model); no V2X security, PKI or closed-loop traffic.
**Novelty implications:** Shows learned radio surrogates are active work; a surrogate trained on joint traffic-radio-PKI ground truth with counterfactual labels is not addressed.
