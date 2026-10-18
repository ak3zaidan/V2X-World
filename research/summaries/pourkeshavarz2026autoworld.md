# AutoWorld: Learning Multi-Agent Traffic Simulation with Self-Supervised World Models
Mozhgan Pourkeshavarz, Tianran Liu, Nicholas Rhinehart · arXiv 2026 · https://arxiv.org/abs/2603.28963 · relevance: low · basis: abstract

**Problem:** Simulation with realistic traffic agents is essential for validating autonomous driving systems.
**Approach:** To address this limitation, we propose AutoWorld, a traffic simulation framework that grounds agent behavior in raw sensor observations through a self-supervised world model trained on LiDAR occupancy data. Given world model samples, AutoWorld constructs a coarse-to-fine predictive scene context as input to a multi-agent motion generation model.
**Tools and scale:** Mentions: LiDAR.
**Results:** Experiments on the Waymo Sim Agents Challenge (WOSAC) demonstrate that AutoWorld achieves competitive performance, with larger gains in partially-observed scenarios where trajectory abstractions are most limited. We further show that grounding simulation in raw LiDAR through AutoWorld scales better with additional data than trajectory-only and LiDAR-conditioning baselines.
**Limitations and future work (as stated by the authors):**
- "To address this limitation, we propose AutoWorld, a traffic simulation framework that grounds agent behavior in raw sensor observations through a self-supervised world model trained on LiDAR occupancy data."
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
