# A Synthetic Benchmark for Collaborative 3D Semantic Occupancy Prediction in V2X-Enabled Autonomous Driving
Hanlin Wu, Pengfei Lin, Ehsan Javanmardi, Naren Bao, Bo Qian, Hao Si et al. · arXiv 2025 · https://doi.org/10.1109/icra57385.2026.11697018 · relevance: medium · basis: abstract

**Problem:** 3D semantic occupancy prediction is an emerging perception paradigm in autonomous driving, providing a voxel-level representation of both geometric details and semantic categories.
**Approach:** However, despite its fine-grained scene understanding, its effectiveness is inherently constrained in single-vehicle setups by occlusions, restricted sensor range, and narrow viewpoints. To address these limitations, collaborative perception enables the exchange of complementary information, thereby enhancing the completeness and accuracy of predictions.
**Tools and scale:** Mentions: CARLA; scale not stated in the abstract.
**Results:** Experimental results demonstrate the superior performance of our baseline enabled by vehicle collaboration, with increasing gains observed as the prediction range expands.
**Limitations and future work (as stated by the authors):**
- To address these limitations, collaborative perception enables the exchange of complementary information, thereby enhancing the completeness and accuracy of predictions.
**Overlap with our engine:** partial — addresses V2X simulation/datasets (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
