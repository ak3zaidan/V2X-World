# NetArena: Dynamic Benchmarks for AI Agents in Network Automation
Yajie Zhou, Jia-Jun Ruan, Eric Wang, Sadjad Fouladi, Francis Y. Yan, Kevin Hsieh et al. · n/a 2025 · https://arxiv.org/abs/2506.03231 · relevance: high · basis: full text (extracted sections)

**Problem:** Static AI-agent benchmarks for network operations risk contamination, high variance and unrealism.
**Approach:** NetArena: dynamic benchmark generation with a state-action abstraction, automatic ground truth and emulator-integrated checks of correctness, safety and latency.
**Tools and scale:** Network emulators; three applications.
**Results:** Confidence-interval overlap reduced from 85% to 0; agents achieve only 13-38% average (as low as 3%) on large realistic queries.
**Limitations and future work (as stated by the authors):**
- Future use: RL post-training environments since many tasks lack step-level ground truth.
**Overlap with our engine:** partial - simulator-backed benchmark for LLM agents, similar in spirit to our LLM agent plus ground truth; not V2X.
**Novelty implications:** Dynamic emulator-based agent benchmarks exist for networks; V2X cross-layer root-cause agent benchmarks with simulator ground truth remain open.
