# ICR-Drive: Instruction Counterfactual Robustness for End-to-End Language-Driven Autonomous Driving
Kaiser Hamid, Can Cui, Nade Liang · Proceedings of the IEEE/CVF Conference on Computer Vision and Pattern Recognition (CVPR) Workshops, 2026, pp. 872-880 2026 · https://arxiv.org/abs/2604.05378 · relevance: low · basis: abstract

**Problem:** Recent progress in vision-language-action (VLA) models has enabled language-conditioned driving agents to execute natural-language navigation commands in closed-loop simulation, yet standard evaluations largely assume instructions are precise and well-formed.
**Approach:** We introduce ICR-Drive, a diagnostic framework for instruction counterfactual robustness in end-to-end language-conditioned autonomous driving.
**Tools and scale:** Mentions: CARLA.
**Results:** Experiments on LMDrive and BEVDriver show that minor instruction changes can induce substantial performance drops and distinct failure modes, revealing a reliability gap for deploying embodied foundation models in safety-critical driving.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
