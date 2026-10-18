# FaulT-Bench: Towards Benchmarking Network Troubleshooting LLM Agents under Unreliable User Tickets
Kuan-Hao Tseng, Niruth Bogahawatta, Yasod Ginige, Kunjan Patel, Kosta Dakic, Suranga Seneviratne · arXiv 2026 · https://arxiv.org/abs/2608.27021 · relevance: high · basis: full text (extracted sections)

**Problem:** Troubleshooting benchmarks assume accurate tickets and a present fault.
**Approach:** FaulT-Bench: 200 scenarios on eight topologies with genuine faults and false/misattributed tickets; persona rewrites; Kathará deployment, NIKA tools, LLM judge.
**Tools and scale:** Kathará emulation; SADE, ReAct, Claude Code.
**Results:** Agents near-saturated on accurate tickets but degrade sharply when network is healthy and ticket wrong; vague tickets hurt more than confidently wrong ones.
**Limitations and future work (as stated by the authors):**
- LLM-judge scoring of free-text diagnoses.
**Overlap with our engine:** partial - root-cause analysis agents benchmark (runner-up LLM RCA direction).
**Novelty implications:** False-premise robustness of RCA agents is studied in IP networks; V2X cross-layer versions remain open.
