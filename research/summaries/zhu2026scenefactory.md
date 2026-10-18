# SceneFactory-3D: Lifting 2D Traffic Scenes into 3D Physical Counterfactuals for Scalable Physically Grounded Safety Evaluation
Yicheng Zhu, Linfeng Tian, Tianmu Zhao et al. · arXiv 2026 · https://arxiv.org/abs/2610.02874 · relevance: low · basis: abstract

**Problem:** Scalable driving simulators typically execute vehicle commands using prescribed behavioral or kinematic rules, overlooking the physics of tire-road interfaces, thereby limiting their ability to capture how adverse road and environmental conditions alter vehicle execution and propagate through traffic.
**Approach:** To address this limitation, we present SceneFactory-3D, a GPU-batched, physics-grounded multi-agent driving simulator.
**Tools and scale:** Not stated in the abstract.
**Results:** Per-world terrain isolation and GPU batching enable SceneFactory-3D to run matched physical counterfactuals in parallel: traffic scenario setup and vehicle controllers remain fixed while only the road condition changes, enabling the resulting closed-loop effects to be evaluated across parallel worlds.
**Limitations and future work (as stated by the authors):**
- "To address this limitation, we present SceneFactory-3D, a GPU-batched, physics-grounded multi-agent driving simulator."
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
