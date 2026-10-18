# MDrive: Benchmarking Closed-Loop Cooperative Driving for End-to-End Multi-agent Systems
Marco Coscoy, Zewei Zhou, Seth Z. Zhao et al. · arXiv 2026 · https://arxiv.org/abs/2605.10904 · relevance: low · basis: abstract

**Problem:** Vehicle-to-Everything (V2X) communication has emerged as a promising paradigm for autonomous driving, enabling connected agents to share complementary perception information and negotiate with each other to benefit the final planning.
**Approach:** In this paper, we introduce MDrive, a closed-loop cooperative driving benchmark comprising 225 scenarios grounded in both NHTSA pre-crash typologies and real-world V2X datasets.
**Tools and scale:** Not stated in the abstract.
**Results:** Our benchmark results demonstrate that multi-agent systems are generally better than single-agent counterparts. However, current multi-agent systems still face two important challenges: (i) perception sharing enhances perceptions, but doesn't always translate to better planning; (ii) negotiation improves planning performance but harms it in complex and dense traffic scenarios.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
