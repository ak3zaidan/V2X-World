# SimTIO: A Simulation-Grounded Multi-Agent LLM Framework for Compositional Traffic Intervention Optimization
Shuyang Li, Ruimin Ke · arXiv 2026 · https://arxiv.org/abs/2609.05740 · relevance: medium · basis: abstract

**Problem:** Traffic analysts must translate diagnosed bottlenecks into executable interventions without allowing local improvements to degrade network-wide performance. This study presents SimTIO, a simulation-grounded multi-agent large language model framework for composing and selecting traffic interventions under explicit operational constraints.
**Approach:** This study presents SimTIO, a simulation-grounded multi-agent large language model framework for composing and selecting traffic interventions under explicit operational constraints.
**Tools and scale:** SimTIO first simulates an unmodified SUMO scenario to identify a baseline-frozen set of ten bottleneck edges.
**Results:** Across 15 cases covering five U.S. urban networks, three synthetic-demand seeds, and 2,400 origin-destination trips per scenario, SimTIO reduced Top-10 bottleneck time loss by an average of 9.18 percent and network-wide delay by 2.78 percent. It found a feasible improving plan in 86.7 percent of cases, compared with 73.3 percent for grounded random search and 80.0 percent for a deterministic heuristic under the same seven-simulation budget, although the differences in Top-10 improvement were not statistically ...
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — LLM multi-agent framework grounded in SUMO simulation feedback.
**Novelty implications:** LLM agents operating a traffic simulator are prior art.
