# Karma: Adaptive Video Streaming via Causal Sequence Modeling
Bowei Xu, Hao Chen, Zhan Ma · arXiv 2023 · https://arxiv.org/abs/2308.10230 · relevance: low · basis: abstract

**Problem:** Optimal adaptive bitrate (ABR) decision depends on a comprehensive characterization of state transitions that involve interrelated modalities over time including environmental observations, returns, and actions.
**Approach:** This paradigm tends to cause a chain of deviations from optimal action when encountering unfamiliar observations, which consequently undermines the model generalization.
**Tools and scale:** Not stated in the abstract.
**Results:** This paper presents Karma, an ABR algorithm that utilizes causal sequence modeling to improve generalization by comprehending the interrelated causality among past observations, returns, and actions and timely refining action when deviation occurs. We evaluate Karma through trace-driven simulations and real-world field tests, demonstrating superior performance compared to existing state-of-the-art ABR algorithms, with an average QoE improvement ranging from 10.8% to 18.7% across diverse network conditions.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
