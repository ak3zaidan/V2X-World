# $α^3$-Bench: A Unified Benchmark of Safety, Robustness, and Efficiency for LLM-Based UAV Agents over 6G Networks
Mohamed Amine Ferrag, Abderrahmane Lakas, Merouane Debbah · arXiv 2026 · https://doi.org/10.1109/ojits.2026.3713296 · relevance: low · basis: abstract

**Problem:** Large Language Models (LLMs) are increasingly being used as high-level controllers for autonomous Unmanned Aerial Vehicle (UAV) missions.
**Approach:** However, current evaluations rarely assess whether such agents remain safe, protocol-compliant, and effective under realistic constraints of next-generation networking. This paper introduces $\alpha ^{3}$ -Bench, a benchmark for assessing LLM-driven UAV autonomy as a multi-turn conversational reasoning and control problem operating under dynamic 6G conditions.
**Tools and scale:** Mentioned in abstract: dataset.
**Results:** Experimental results show that several frontier models achieve near-perfect mission success and safety compliance (Task Outcome and Safety Policy $\geq 0.95$ ), yet their robustness and efficiency diverge substantially: under degraded 6G conditions, Network Robustness scores drop by up to 30–40%, while efficiency-normalized performance varies by more than a $2\times $ factor across models in both $\alpha ^{3}$ per-second and $\alpha ^{3}$ per-1k-tokens metrics, reflecting significant variance in inference latency and token consumption. $\alpha ^{3}$ -Bench provides a reproducible, extensible foundation for benchmarking conversational UAV autonomy and guiding the development of safe, network-aware, and resource-efficient LLM agents in 6G-enabled aerial systems.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — touches vehicular networking, LLM agents but not the joint traffic/radio/SCMS closed loop.
**Novelty implications:** Nothing in our candidate directions is ruled out; useful only as background on vehicular networking.
