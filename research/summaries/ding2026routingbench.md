# RoutingBench: Can Agentic Routing Analysis Scale to Production Datacenter Networks?
Wenlong Ding, Zhixiong Niu, Jianan Yang, Fajun Zhang et al. · arXiv 2026 · https://arxiv.org/abs/2609.22848 · relevance: medium · basis: abstract

**Problem:** Recent advances in AI models and agentic technologies make AI for network operations (NetOps) within reach. However, scalability remains a key bottleneck of agentic NetOps when analyzing hyperscale networks, which comprise hundreds of datacenters, each housing thousands of network devices.
**Approach:** This paper studies this scalability problem and evaluates how different agentic approaches, namely in-context learning, iterative reasoning, and agent skills, can scale routing-path analysis to large, complex networks.
**Tools and scale:** Not stated in the abstract.
**Results:** Our results show that agentic analysis is promising: agent skills curated with a principle termed "explore more; digest less" enables routing-path analysis on hyperscale networks of 50K routers with an accuracy of 99.5%, significantly outreaching the scalability of traditional ...
**Limitations and future work (as stated by the authors):**
- Meanwhile, RoutingBench also reveals the boundary of AI agent capability on complex inter-datacenter networks and compound changes, posing open challenges for AI and agentic research.
**Overlap with our engine:** partial — LLM agents for network/security operations; our engine has an LLM agent that configures and analyses runs, but this work is not V2X and not tied to a deterministic simulator with ground truth.
**Novelty implications:** LLM agents for SOC/NetOps/RCA are established; cross-layer V2X root-cause analysis against simulator ground truth is not covered here.
