# Ufil: A Unified Framework for Infrastructure-based Localization
Simon Schäfer, Lucas Hegerath, Marius Molz, Massimo Marcon, Bassam Alrifaee · arXiv.org 2026 · https://arxiv.org/abs/2604.21471 · relevance: medium · basis: abstract

**Problem:** Infrastructure-based localization enhances road safety and traffic management by providing state estimates of road users.
**Approach:** Development is hindered by fragmented, application-specific stacks that tightly couple perception, tracking, and middleware. We introduce Ufil, a Unified Framework for Infrastructure-Based Localization with a standardized object model and reusable multi-object tracking components.
**Tools and scale:** Mentions: CARLA, testbed; scale not stated in the abstract.
**Results:** Ufil offers interfaces and reference implementations for prediction, detection, association, state update, and track management, allowing researchers to improve components without reimplementing the pipeline. In a three-lane highway scenario with 423 and 355 vehicles in simulation and testbed, respectively, the fused system achieves lane-level lateral accuracy with mean lateral position RMSEs of 0.31 m in CARLA and 0.29 m in the CPM Lab, and mean absolute orientation errors around 2.2{\deg}.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses V2X simulation/datasets (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
