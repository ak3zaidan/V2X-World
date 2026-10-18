# Generating Roadside LiDAR Datasets from Vehicle-Side Datasets via Novel View Synthesis
Yuhan Xia, Runxin Zhao, Hanyang Zhuang, Chunxiang Wang, Ming Yang · arXiv 2026 · https://arxiv.org/abs/2605.05897 · relevance: medium · basis: abstract

**Problem:** Intelligent Transportation Systems (ITS) require reliable environmental perception to support safe and efficient transportation.
**Approach:** With the rapid development of Vehicle-to-everything (V2X), roadside perception has become an effective means to extend sensing coverage and improve traffic safety. However, the scarcity of large-scale annotated roadside LiDAR datasets poses a major challenge for training high-performance roadside perception models.
**Tools and scale:** Mentions: real-world; scale not stated in the abstract.
**Results:** Extensive experiments on roadside 3D object detection demonstrate that the synthesized data effectively complements real roadside data, mitigates the limitations of limited real-world roadside data, and improves generalization to unseen roadside viewpoints.
**Limitations and future work (as stated by the authors):**
- Extensive experiments on roadside 3D object detection demonstrate that the synthesized data effectively complements real roadside data, mitigates the limitations of limited real-world roadside data, and improves generalization to unseen roadside viewpoints.
**Overlap with our engine:** partial — addresses V2X simulation/datasets (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
