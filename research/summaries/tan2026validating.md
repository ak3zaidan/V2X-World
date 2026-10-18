# Validating Causal Message Passing Against Network-Aware Methods on Real Experiments
Albert Tan, Sadegh Shirani, James Nordlund, Mohsen Bayati · arXiv 2026 · https://arxiv.org/abs/2602.04230 · relevance: medium · basis: abstract

**Problem:** Estimating total treatment effects in the presence of network interference typically requires knowledge of the underlying interaction structure. However, in many practical settings, network data is either unavailable, incomplete, or measured with substantial error.
**Approach:** We apply causal message passing to two large-scale field experiments where a recently developed bipartite graph methodology, which requires network knowledge, serves as a benchmark.
**Tools and scale:** Not stated in the abstract.
**Results:** We demonstrate that causal message passing, a methodology that leverages temporal structure in outcome data rather than network topology, can recover total treatment effects comparable to network-aware approaches.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Causal inference under interference, methodologically relevant to counterfactual reruns; no simulator or V2X content.
**Novelty implications:** Provides estimands/identification theory for spillover effects; exact simulator-generated per-unit counterfactuals for V2X remain unaddressed here.
