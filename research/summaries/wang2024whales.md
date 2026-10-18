# WHALES: A Multi-Agent Scheduling Dataset for Enhanced Cooperation in Autonomous Driving
Yinsong Wang, Siwei Chen, Ziyi Song, Sheng Zhou · arXiv 2024 · https://doi.org/10.1109/iros60139.2025.11247472 · relevance: medium · basis: abstract

**Problem:** Cooperative perception research is hindered by the limited availability of datasets that capture the complexity of real-world Vehicle-to-Everything (V2X) interactions, particularly under dynamic communication constraints.
**Approach:** To address this gap, we introduce WHALES (Wireless enhanced Autonomous vehicles with Large number of Engaged agents), the first large-scale V2X dataset explicitly designed to benchmark communication-aware agent scheduling and scalable cooperative perception. WHALES introduces a new benchmark that enables state-of-the-art (SOTA) research in communication-aware cooperative perception, featuring an average of 8.4 cooperative agents per scene and 2.01 million annotated 3D objects across diverse traffic scenarios.
**Tools and scale:** Mentions: dataset, real-world; scale not stated in the abstract.
**Results:** To further advance the field, we propose the Coverage-Aware Historical Scheduler (CAHS), a novel scheduling baseline that selects agents based on historical viewpoint coverage, improving perception performance over existing SOTA methods.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses V2X simulation/datasets (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
