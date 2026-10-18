# CausalDrive: Real-time Causal World Models for Autonomous Driving
Tianyi Yan, Huan Zheng, Dubing Chen et al. · arXiv 2026 · https://arxiv.org/abs/2606.15341 · relevance: low · basis: abstract

**Problem:** World models have emerged as a promising paradigm for scaling autonomous driving (AD) data, yet existing video generative models fall short as interactive simulators.
**Approach:** To bridge this gap, we present CausalDrive, a controllable, real-time foundation driving world renderer. By excluding future NPC layouts, we compel the model to intrinsically predict causal interactions, enabling text-driven control over Driving Sociology, allowing users to dynamically orchestrate diverse counterfactual reactions to identical ego-actions.
**Tools and scale:** Not stated in the abstract.
**Results:** This combines continuous flow-matching with a self-correcting distillation objective, achieving interactive speeds of 12 FPS.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
