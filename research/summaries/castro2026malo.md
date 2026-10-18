# MaLO: Multi-agent LLM Operations for Autonomous Security
Sebastián R. Castro, Iakov Taranenko, Astra Tsai, Nancy Lau et al. · 2026 IEEE Conference on Communications and Network Security (CNS) 2026 · https://doi.org/10.1109/cns69863.2026.11720875 · relevance: medium · basis: abstract

**Problem:** Despite the wide adoption of Large Language Models (LLMs) and AI agents in cybersecurity, the impact of agent topology on autonomous cyber operations (ACO) performance remains underexplored. Current agent-based security systems rely on either monolithic LLM prompts or rigid, static pipelines.
**Approach:** We present MaLO, a configurable security platform to formalize and evaluate dynamic multi-agent graphs.
**Tools and scale:** We evaluate these architectures across a 42-task benchmark of varying difficulty quantified by our new SOCX metric, and a highly realistic, long-horizon security scenario (NSA CBC) that mirrors realworld task ambiguity.
**Results:** Our results demonstrate that explicit task decomposition and orchestration significantly improve performance on medium and high-complexity security operations. By achieving a 78.6% success rate in our benchmark and solving tasks up to 40x faster than a human expert baseline, we show that architecture selection is a first-class design decision for autonomous cybersecurity agents.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — LLM agents for network/security operations; our engine has an LLM agent that configures and analyses runs, but this work is not V2X and not tied to a deterministic simulator with ground truth.
**Novelty implications:** LLM agents for SOC/NetOps/RCA are established; cross-layer V2X root-cause analysis against simulator ground truth is not covered here.
