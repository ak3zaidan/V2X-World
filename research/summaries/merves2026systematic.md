# Systematic Capability Benchmarking of Frontier Large Language Models for Offensive Cyber Tasks
Tyler H. Merves, Michael H. Conaway, Joseph M. Escobar, Hakan T. Otal et al. · arXiv 2026 · https://arxiv.org/abs/2604.17159 · relevance: medium · basis: abstract

**Problem:** We present, to our knowledge, the most comprehensive cross-model evaluation of LLM agents on offensive cybersecurity tasks, benchmarking 10 frontier models from 7 providers on all 200 challenges of the NYU CTF Bench. Building on the D-CIPHER multi-agent framework, we extend it with multi-provider backend support, a custom Kali Linux environment with over ...
**Approach:** We present, to our knowledge, the most comprehensive cross-model evaluation of LLM agents on offensive cybersecurity tasks, benchmarking 10 frontier models from 7 providers on all 200 challenges of the NYU CTF Bench.
**Tools and scale:** Not stated in the abstract.
**Results:** Among models, Claude 4.5 Opus achieves the highest solve rate (59%), followed by Gemini 3 Pro (52%), with Gemini 3 Flash offering the best cost-efficiency at $0.05 per solve. Asymmetric planner/executor model assignments provide no meaningful benefit while coherent same-model configurations consistently outperform mixed-tier pairings.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — LLM agents for network/security operations; our engine has an LLM agent that configures and analyses runs, but this work is not V2X and not tied to a deterministic simulator with ground truth.
**Novelty implications:** LLM agents for SOC/NetOps/RCA are established; cross-layer V2X root-cause analysis against simulator ground truth is not covered here.
