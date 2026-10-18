# NetAgentBench: A State-Centric Benchmark for Evaluating Agentic Network Configuration
Ahmed Twabi, Yepeng Ding, Tohru Kondo · arXiv.org 2026 · https://arxiv.org/abs/2604.09678 · relevance: high · basis: full text (extracted sections)

**Problem:** Static one-shot tests miss multi-turn agent behaviour in network configuration.
**Approach:** NetAgentBench: FSM-formalised dynamic benchmark guaranteeing determinism, correctness and bounded execution.
**Tools and scale:** Four LLM agents; OSPF and other configuration tasks.
**Results:** Agents solve basic tasks but show exploration meltdown and coherence collapse at expert level; e.g., Qwen3-Coder-30b exploration ratio 1.688.
**Limitations and future work (as stated by the authors):**
- Golden-configuration benchmarks risk contamination.
**Overlap with our engine:** partial - deterministic benchmark for multi-turn LLM agents in networking.
**Novelty implications:** Deterministic agent benchmarks exist; deterministic V2X simulator as agent environment is a different domain.
