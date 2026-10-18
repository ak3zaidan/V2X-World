# UrbanTwin: Building High-Fidelity Digital Twins for Sim2Real LiDAR Perception and Evaluation
Muhammad Shahbaz, Shaurya Agarwal · arXiv 2025 · https://arxiv.org/abs/2509.02903 · relevance: medium · basis: abstract

**Problem:** LiDAR-based perception in intelligent transportation systems (ITS) relies on deep neural networks trained with large-scale labeled datasets.
**Approach:** However, creating such datasets is expensive, time-consuming, and labor-intensive, limiting the scalability of perception systems. Sim2Real learning offers a scalable alternative, but its success depends on the simulation's fidelity to real-world environments, dynamics, and sensors.
**Tools and scale:** Mentions: real-world; scale not stated in the abstract.
**Results:** Using this workflow, we have released three synthetic LiDAR datasets, namely UT-LUMPI, UT-V2X-Real, and UT-TUMTraf-I, which closely replicate real locations and outperform real-data-trained baselines in perception tasks.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses V2X simulation/datasets (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
