# TerrainForge: Physics-Grounded Road Geometry Editing for Counterfactual Autonomous Driving
Yang Chen, Yicheng Zhu, zhenning Li et al. · arXiv 2026 · https://arxiv.org/abs/2610.02825 · relevance: low · basis: abstract

**Problem:** Road geometry (e.g., crests, sags, and speed humps) and surface conditions (e.g., wet or icy pavement) affect how vehicles move, what drivers and onboard cameras observe, and how much clearance remains between vehicles.
**Approach:** We present TerrainForge, a framework for generating road geometry-focused counterfactuals from reconstructed multi-vehicle driving episodes. A unified road model connects scene deformation with four-wheel vehicle dynamics, allowing crests, sags, speed humps, and friction changes to propagate through vehicle motion, camera viewpoint, and inter-vehicle clearance.
**Tools and scale:** Not stated in the abstract.
**Results:** These pairs train a first-stage screening surrogate that takes the original driving context and candidate road-edit parameters as input and predicts the resulting change in the ego's terminal gap. On held-out scenes, this prediction achieves 22-40% lower mean absolute error than predicting no change, so candidates can be screened cheaply before the full multi-vehicle rollout.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
