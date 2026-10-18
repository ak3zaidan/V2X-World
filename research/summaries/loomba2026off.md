# Off-policy causal estimation in networks
Sahil Loomba, Dean Eckles · arXiv 2026 · https://arxiv.org/abs/2609.02756 · relevance: medium · basis: abstract

**Problem:** In the presence of interference, where the treatment assigned to one unit can affect the outcomes of others, many causal estimands depend on the treatment-assignment policy under which the experiment is conducted. This policy dependence creates a fundamental challenge for off-policy estimation, where the goal is to estimate causal quantities under a ...
**Approach:** We study this problem of off-policy estimation of causal effects for heterogeneous Bernoulli policies.
**Tools and scale:** Simulations illustrate these theoretical results for the design and analysis of experiments under network interference and design mismatch.
**Results:** By representing exposure-weighted potential outcomes in the biased Fourier basis of the experimental design, we construct, for any prespecified Fourier subspace encoding the assumed interference structure, the unique minimum-$L^2$ weight that transports every function in that ...
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Causal inference under interference, methodologically relevant to counterfactual reruns; no simulator or V2X content.
**Novelty implications:** Provides estimands/identification theory for spillover effects; exact simulator-generated per-unit counterfactuals for V2X remain unaddressed here.
