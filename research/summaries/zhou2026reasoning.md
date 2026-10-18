# Reasoning Topology Matters: A Controlled Study of LLM-Based Cybersecurity Analysis
Jiling Zhou, Aisvarya Adeseye, Antti Hakkala, Seppo Virtanen et al. · arXiv 2026 · https://arxiv.org/abs/2609.24710 · relevance: medium · basis: abstract

**Problem:** Large Language Models (LLMs) are increasingly used in cybersecurity, where accurate analysis often requires multi-step and context-dependent reasoning over complex and heterogeneous data. However, existing prompting approaches typically focus on eliciting reasoning without explicitly considering how intermediate reasoning steps are structurally organized.
**Approach:** We introduce Security Reasoning Topology, which models reasoning through three representative structures: Linear, Branching, and Graph.
**Tools and scale:** Not stated in the abstract.
**Results:** Results show that reasoning topology substantially affects performance: Graph reasoning achieves the highest overall accuracy, improving over few-shot prompting by 9.8-12.2 percentage points across datasets, while Branching provides a strong intermediate solution. The results further show that the effect of reasoning topology remains consistent across model families and scales, highlighting reasoning topology as an important design factor for LLM-based cybersecurity analysis.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — LLM agents for network/security operations; our engine has an LLM agent that configures and analyses runs, but this work is not V2X and not tied to a deterministic simulator with ground truth.
**Novelty implications:** LLM agents for SOC/NetOps/RCA are established; cross-layer V2X root-cause analysis against simulator ground truth is not covered here.
