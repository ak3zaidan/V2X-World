# Agentic Security: A Systematization of Tools, Failure Modes, and Design Laws for LLM-Driven Penetration Testing
Israt Moyeen Noumi, Tarannum Ahmed Nowshin, Md. Mehedi Hasan Nipu et al. · arXiv 2026 · https://arxiv.org/abs/2608.21423 · relevance: low · basis: abstract

**Problem:** Agentic security uses large-language-model (LLM) agents to plan, dispatch, and interpret security tools.
**Approach:** We introduce a four-dimensional Integration Friction Index that separates one-time engineering cost from recurring organisational, legal, and maintenance cost. We formulate planner-versus-worker model routing as a knapsack problem and derive a closed-form execution cap for heavy-tailed tools, eta* = alpha v/c.
**Tools and scale:** Not stated in the abstract.
**Results:** Modelling an agentic security system as stochastic LLM policies wrapped by a deterministic mediator, we show that long-lived sessions lose resident evidence with phase count, while short-lived sub-agents extend the usable horizon according to the compression ratio between raw evidence and its summary. We show that a two-stage verdict cascade multiplies scorer likelihood ratios, but provides little benefit when scorer errors correlate.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Studies V2X attacks or their detection; overlaps the misbehaviour/attack modules but without linkage-based revocation, closed loop to the SCMS, or joint ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
