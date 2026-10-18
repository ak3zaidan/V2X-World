# C$^2$T: Captioning-Structure and LLM-Aligned Common-Sense Reward Learning for Traffic--Vehicle Coordination
Yuyang Chen, Kaiyan Zhao, Yiming Wang et al. · arXiv 2026 · https://arxiv.org/abs/2604.13098 · relevance: low · basis: abstract

**Problem:** State-of-the-art (SOTA) urban traffic control increasingly employs Multi-Agent Reinforcement Learning (MARL) to coordinate Traffic Light Controllers (TLCs) and Connected Autonomous Vehicles (CAVs).
**Approach:** To overcome this limitation, we introduce C2T, a novel framework that learns a common-sense coordination model from traffic-vehicle dynamics. C2T distills "common-sense" knowledge from a Large Language Model (LLM) into a learned intrinsic reward function.
**Tools and scale:** Not stated in the abstract.
**Results:** Our framework significantly outperforms strong MARL baselines in traffic efficiency, safety, and an energy-related proxy.
**Limitations and future work (as stated by the authors):**
- "To overcome this limitation, we introduce C2T, a novel framework that learns a common-sense coordination model from traffic-vehicle dynamics."
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
