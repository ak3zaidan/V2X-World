# SecRespond: Benchmarking AI Agents for Real-World Post-Compromise Incident Response
Lehan Wang, Boli Chen, Ruixue Ding, Pengjun Xie et al. · arXiv 2026 · https://arxiv.org/abs/2607.26791 · relevance: medium · basis: abstract

**Problem:** Large Language Model (LLM) agents are increasingly adopted in real-world security operations with access to host artifacts and command-line interfaces (CLIs), making it critical to thoroughly assess their security capabilities. However, existing cybersecurity benchmarks focus on pre-compromise settings where agents are placed in a clean and idealized ...
**Approach:** To address this gap, we introduce SecRespond, the first benchmark for evaluating LLM agents on the post-compromise incident-response workflow.
**Tools and scale:** Large Language Model (LLM) agents are increasingly adopted in real-world security operations with access to host artifacts and command-line interfaces (CLIs), making it critical to thoroughly assess their security capabilities.
**Results:** We evaluate 23 frontier LLMs on the OpenCode agent harness. Experimental results show that although current agents can reliably uncover the problems exposed by alerts, they struggle to proactively investigate the disk for silent intrusions and to produce comprehensive, verified remediation plans, with no model achieving complete ...
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — LLM agents for network/security operations; our engine has an LLM agent that configures and analyses runs, but this work is not V2X and not tied to a deterministic simulator with ground truth.
**Novelty implications:** LLM agents for SOC/NetOps/RCA are established; cross-layer V2X root-cause analysis against simulator ground truth is not covered here.
