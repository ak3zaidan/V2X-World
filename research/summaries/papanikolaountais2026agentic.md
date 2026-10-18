# Agentic-V2X: Small Language Model Agents for Deadline-Aware V2X Scheduling in 5G/6G Networks
Gerasimos Papanikolaou-Ntais, Alexandros Kaloxylos, Athanasios Kanavos · arXiv 2026 · https://arxiv.org/abs/2607.04290 · relevance: high · basis: full text (extracted sections)

**Problem:** LLMs are too slow and unreliable to schedule packets in real time.
**Approach:** Small local LM generates validated policies (rApp-like); deterministic xApp-like controller enforces scheduler weights.
**Tools and scale:** ns-3, 5G-LENA, SUMO, ns3-ai; 126 runs; heterogeneous V2X services.
**Results:** Adaptive LLM policies valid and competitive; e.g., +5.6 pp mean critical DC-PRR at 30 vehicles vs PF (CI includes zero).
**Limitations and future work (as stated by the authors):**
- Not a real deployed O-RAN rApp/xApp.
- Several differences not statistically significant.
**Overlap with our engine:** partial - LLM in the loop of an ns-3/SUMO V2X simulation.
**Novelty implications:** LLM policy generation for V2X scheduling exists; LLM security analysis does not.
