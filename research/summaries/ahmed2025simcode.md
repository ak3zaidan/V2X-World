# SIMCODE: A Benchmark for Natural Language to ns-3 Network Simulation Code Generation
Tasnim Ahmed, Mirza Mohammad Azwad, Salimur Choudhury · 2025 IEEE 50th Conference on Local Computer Networks (LCN) 2025 · https://doi.org/10.1109/lcn65610.2025.11146057 · relevance: high · basis: full text (extracted sections)

**Problem:** LLM ability to write ns-3 simulation code is underexplored.
**Approach:** SIMCODE: 400 NL-to-ns-3 tasks with verified solutions and tests; evaluation of three LLMs and six prompt techniques plus fine-tuning.
**Tools and scale:** ns-3 >= 3.41; Gemini-2.0, GPT-4.1, Qwen-3.
**Results:** GPT-4.1 best but execution accuracy modest; missing headers and API mismatches dominate failures.
**Limitations and future work (as stated by the authors):**
- Substantial room for improvement in execution accuracy.
**Overlap with our engine:** partial - LLMs producing simulator configurations.
**Novelty implications:** LLM-for-simulator benchmarks exist; config-level agents over a single deterministic V2X engine remain distinct.
