# Hierarchical Agentic Incident Response with Digital-Twin-Validated Attack Inference
Yiran Gao, Juntao Chen, Tao Li · arXiv 2026 · https://arxiv.org/abs/2608.15016 · relevance: medium · basis: abstract

**Problem:** Network incident response remains slow and labor-intensive as the defender must infer multi-stage attacks from partial observations and translate recovery decisions into reliable system commands. Decision-theoretic planners provide principled optimization but typically rely on abstract states and predefined actions, while large language model (LLM) agents ...
**Approach:** Toward automating response planning, we present a hierarchical agentic response framework that integrates LLM-based attack inference, rollout planning, and digital-twin validation.
**Tools and scale:** We evaluate the framework on a 33-component enterprise-network testbed under three multi-stage attack scenarios.
**Results:** We evaluate the framework on a 33-component enterprise-network testbed under three multi-stage attack scenarios. The results show that our framework outperforms frontier-LLM baselines in recovery success rate by 18--31%.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — LLM agents for network/security operations; our engine has an LLM agent that configures and analyses runs, but this work is not V2X and not tied to a deterministic simulator with ground truth.
**Novelty implications:** LLM agents for SOC/NetOps/RCA are established; cross-layer V2X root-cause analysis against simulator ground truth is not covered here.
