# Mitigating Anchoring Bias in LLM-Based Agents for Energy-Efficient 6G Autonomous Networks
Hatim Chergui, Claudia Carballo González, Farhad Rezazadeh et al. · arXiv 2026 · https://arxiv.org/abs/2606.18272 · relevance: low · basis: abstract

**Problem:** This paper presents an autonomous agentic resource negotiation framework designed to enable zero-touch network slicing in 6G architectures using Large Language Model (LLM) agents.
**Approach:** To systematically mitigate this cognitive bias, we propose a novel randomized anchoring strategy modeled via a Truncated 3-Parameter Weibull distribution. This mathematically bounded approach seamlessly integrates with burst-aware Digital Twins (DTs) employing Conditional Value at Risk (CVaR) to rigorously guarantee strict Service Level Agreement (SLA) tail-latencies.
**Tools and scale:** Not stated in the abstract.
**Results:** Empirical results generated using a locally hosted 1B-parameter model otel-llm-1b-it confirm these dual-regime bounds. Crucially, the lightweight 1B LLM achieves sub-second inference latencies (0.95s mean), ensuring our multi-agent framework is compatible with the operational timescales of the O-RAN non-Real-Time RAN Intelligent Controller (non-RT RIC)\footnote{Our source code is available for non-commercial use at https://github.com/HatimChergui.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
