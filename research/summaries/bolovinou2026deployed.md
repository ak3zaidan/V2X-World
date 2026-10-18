# A Deployed Hybrid Vehicle-in-the-Loop Platform for Validating Cooperative Perception
Anastasia Bolovinou, Giorgos Hadjipavlis, Markos Antonopoulos, Panagiotis Tachtalis, Konstantinos Petousakis, Konstantinos Lazaridis et al. · arXiv 2026 · https://arxiv.org/abs/2607.13806 · relevance: medium · basis: abstract

**Problem:** European safety regulation now permits a large share of automated-driving homologation evidence to be produced virtually, provided a validated physical-virtual facility generates it.
**Approach:** We present a deployed hybrid Vehicle-in-the-Loop (ViL) platform that couples a real instrumented vehicle with a CARLA-based digital twin (DT) through a V2X message pipeline, and we report its first integrated operation on a public-road-representative test track. A real vehicle streams ETSI-compliant CAM/CPM messages into the DT, where a GPU-accelerated Cooperative Perception (CP) module fuses them into a probabilistic occupancy grid during scenario runtime.
**Tools and scale:** Mentions: CARLA; scale not stated in the abstract.
**Results:** The results show that CP substantially widens field-of-view (FoV) coverage and improves occupied-cell recall, and that beyond a moderate localization-noise threshold, positioning uncertainty, and not weather, becomes the dominant error source.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses V2X simulation/datasets (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
