# Benchmarking LLM-Driven Network Configuration Repair
Ioannis Protogeros, Rufat Asadli, Benjamin Hoffman, Laurent Vanbever · arXiv 2026 · https://arxiv.org/abs/2604.22513 · relevance: high · basis: full text (extracted sections)

**Problem:** No benchmark tests whether LLMs can repair large interdependent network configurations without regressions.
**Approach:** Cornetto: generation pipeline synthesising misconfiguration scenarios and formal-verification-based evaluation against ground-truth intents.
**Tools and scale:** 231 problems on topologies of 20-754 nodes; 9 LLMs.
**Results:** LLMs show promise but often introduce regressions and degrade at scale.
**Limitations and future work (as stated by the authors):**
- Evaluates intrinsic LLM capability only; RAG, prompting strategies and agentic systems left for future evaluation.
- Reliable automation requires iterative workflows guided by formal verification.
**Overlap with our engine:** partial - ground-truth-verified LLM benchmark for network operations; not V2X.
**Novelty implications:** Ground-truth-checked LLM network-ops benchmarks exist; V2X cross-layer equivalents remain open.
