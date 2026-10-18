# World Models as Adversaries: Multi-Agent Self-Play Fine-Tuning for Robust Motion Planning
Tong Nie, Yuewen Mei, Junlin He et al. · arXiv 2026 · https://arxiv.org/abs/2607.10630 · relevance: low · basis: abstract

**Problem:** Robust motion planning in dense traffic requires autonomous vehicles to interact in rare and safety-critical scenarios that are underrepresented in naturalistic driving data.
**Approach:** Here, we cast adversarially robust planner learning as a constrained min-max game and propose Adversarial World Modeling (AWM), a theoretically grounded multi-agent self-play fine-tuning framework. In the inner minimization, the planner's predictive world model is converted into a role-conditioned adversary that learns sparse, scene-adaptive attack coalitions via counterfactual credit assignment.
**Tools and scale:** Not stated in the abstract.
**Results:** In the outer maximization, the ego planner optimizes a regret-aware robust best response against the frozen AWM, utilizing tail-risk weighting and reference-anchored trust regions to improve hard-case recovery while preserving nominal driving behavior. Experiments on the nuPlan and InterPlan benchmarks demonstrate that our method generates transferable adversarial interactions and yields a robust planner that achieves competitive closed-loop performance in both nominal and highly interactive long-tail scenarios.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Studies V2X attacks or their detection; overlaps the misbehaviour/attack modules but without linkage-based revocation, closed loop to the SCMS, or joint ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
