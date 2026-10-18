# Agentic AI Empowered Intent-Based Networking for 6G
Genze Jiang, Kezhi Wang, Xiaomin Chen et al. · arXiv 2026 · https://arxiv.org/abs/2601.06640 · relevance: low · basis: abstract

**Problem:** The transition towards sixth-generation (6G) wireless networks necessitates autonomous orchestration mechanisms capable of translating high-level operational intents into executable network configurations.
**Approach:** This paper presents a hierarchical multi-agent framework where Large Language Model (LLM) based agents autonomously decompose natural language intents, consult domain-specific specialists, and synthesise technically feasible network slice configurations through iterative reasoning-action (ReAct) cycles. The proposed architecture employs an orchestrator agent coordinating two specialist agents, i.e., Radio Access Network (RAN) and Core Network agents, via ReAct-style reasoning, grounded in structured network state representations.
**Tools and scale:** Not stated in the abstract.
**Results:** Experimental evaluation across diverse benchmark scenarios shows that the proposed system outperforms rule-based systems and direct LLM prompting, with architectural principles applicable to Open RAN (O-RAN) deployments. The results also demonstrate that whilst contemporary LLMs possess general telecommunications knowledge, network automation requires careful prompt engineering to encode context-dependent decision thresholds, advancing autonomous orchestration capabilities for next-generation wireless systems.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
