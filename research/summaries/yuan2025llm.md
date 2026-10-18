# LLM-Driven Offloading Decisions for Edge Object Detection in Smart City Deployments
Xingyu Yuan, He Li · Smart Cities 2025 · https://doi.org/10.3390/smartcities8050169 · relevance: low · basis: abstract

**Problem:** Object detection is a critical technology for smart city development.
**Approach:** As request volumes surge, inference is increasingly offloaded from centralized clouds to user-proximal edge sites to reduce latency and backhaul traffic. However, heterogeneous workloads, fluctuating bandwidth, and dynamic device capabilities make offloading and scheduling difficult to optimize in edge environments.
**Tools and scale:** Mentioned in abstract: dataset, simulation, simulations.
**Results:** In simulations for a real-world dataset, policies trained with LLM-generated rewards adapt from prompts alone and outperform counterparts trained with expert-designed rewards, while eliminating manual reward engineering.
**Limitations and future work (as stated by the authors):**
- To address this limitation, we introduce an LLM-driven framework that retargets DRL policies for edge object detection directly through natural language instructions.
**Overlap with our engine:** partial — touches network/traffic simulation, LLM agents but not the joint traffic/radio/SCMS closed loop.
**Novelty implications:** Nothing in our candidate directions is ruled out; useful only as background on network/traffic simulation.
