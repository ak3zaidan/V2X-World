# LLM-Based Agentic Negotiation for 6G: Addressing Uncertainty Neglect and Tail-Event Risk
Hatim Chergui, Farhad Rezazadeh, Mehdi Bennis et al. · arXiv 2025 · https://arxiv.org/abs/2511.19175 · relevance: low · basis: abstract

**Problem:** A critical barrier to the trustworthiness of sixth-generation (6G) agentic autonomous networks is the uncertainty neglect bias; a cognitive tendency for large language model (LLM)-powered agents to make high-stakes decisions based on simple averages while ignoring the tail risk of extreme events.
**Approach:** This paper proposes an unbiased, risk-aware framework for agentic negotiation, designed to ensure robust resource allocation in 6G network slicing. Specifically, agents leverage Digital Twins (DTs) to predict full latency distributions, which are then evaluated using a formal framework from extreme value theory, namely, Conditional Value-at-Risk (CVaR).
**Tools and scale:** Not stated in the abstract.
**Results:** The results demonstrate the profound failure of the biased, mean-based baseline, which systematically violates the strict URLLC SLA 11 times. Our unbiased, CVaR-aware agent successfully mitigates this bias, eliminating SLA violations entirely and significantly reducing the 99.999th-percentile latencies by up to 51.7\%.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
