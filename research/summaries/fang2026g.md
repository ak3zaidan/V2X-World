# G-Agent: A Large Language Model-Driven Multi-Agent Framework for Automated Configuration Optimization of Scramjet Cavity Combustors
Yuchen Fang, Dapeng Xiong, Guoyan Zhao, Yixin Yang et al. · Aerospace 2026 · https://doi.org/10.3390/aerospace13090790 · relevance: medium · basis: abstract

**Problem:** This paper presents a G-Agent, a large language model (LLM)-driven multi-agent framework that automates the optimization workflow for an axisymmetric cavity-based scramjet combustor. The framework integrates a supersonic compressible reacting flow solver with the Qwen3-VL-2B-Instruct model.
**Approach:** This paper presents a G-Agent, a large language model (LLM)-driven multi-agent framework that automates the optimization workflow for an axisymmetric cavity-based scramjet combustor.
**Tools and scale:** The Interactor parses user requirements and generates simulation input files.
**Results:** Starting from a baseline thrust of 489.4 N, the optimized configuration achieved 523.5 N after three iterations, reaching a relative improvement of 6.97%. Our proposed agent achieves closed-loop control of the entire optimization process, significantly lowering the technical barrier and human effort in scramjet combustor design.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — LLM agents for network/security operations; our engine has an LLM agent that configures and analyses runs, but this work is not V2X and not tied to a deterministic simulator with ground truth.
**Novelty implications:** LLM agents for SOC/NetOps/RCA are established; cross-layer V2X root-cause analysis against simulator ground truth is not covered here.
