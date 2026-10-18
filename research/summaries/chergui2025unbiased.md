# Toward an Unbiased Collective Memory for Efficient LLM-Based Agentic 6G Cross-Domain Management
Hatim Chergui, Farhad Rezazadeh, Miguel Catalan Cid et al. · arXiv 2025 · https://arxiv.org/abs/2509.26200 · relevance: low · basis: abstract

**Problem:** Agentic artificial intelligence is a candidate enabler of Level-4 autonomy in sixth-generation (6G) networks, but agents reasoning over a shared memory inherit its distortions.
**Approach:** We study cross-domain radio access network (RAN)--edge orchestration in which a RAN agent minimizing energy and an edge agent minimizing latency negotiate, validate proposals against a digital twin (DT), and share a collective memory.
**Tools and scale:** Not stated in the abstract.
**Results:** We define the Retrieval Bias Index, a divergence between the induced retrieval law and a representative reference, decomposed along temporal, confirmation and availability axes, and show that additive retrieval scoring is an exponential tilt of the undebiased law, yielding a closed form for the failure-amplification weight and a saturation floor set by the store's own composition; a further result bounds excess service level agreement (SLA) violation additively in retrieval bias and DT mismatch. On a tandem queue for which stochastic network calculus supplies a feasible region, an oracle and a Nash bargaining reference, the operator attains a $12.7\times$ bias reduction over a recency store and the smallest distance to the bargaining solution among six memory architectures.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
