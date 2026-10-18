# TrafficLLM: Enhancing Large Language Models for Network Traffic Analysis with Generic Traffic Representation
Tianyu Cui, Xinjie Lin, Sijia Li, Miao Chen et al. · arXiv 2025 · https://arxiv.org/abs/2504.04222 · relevance: medium · basis: abstract

**Problem:** Machine learning (ML) powered network traffic analysis has been widely used for the purpose of threat detection. Unfortunately, their generalization across different tasks and unseen data is very limited.
**Approach:** To address the issue, in this paper, we propose TrafficLLM, which introduces a dual-stage fine-tuning framework to learn generic traffic representation from heterogeneous raw traffic data. We evaluate TrafficLLM across 10 distinct scenarios and 229 types of traffic.
**Tools and scale:** Mentioned in abstract: LLM, real-world. (From the abstract only.)
**Results:** TrafficLLM achieves F1-scores of 0.9875 and 0.9483, with up to 80.12% and 33.92% better performance than existing detection and generation methods. It also shows strong generalization on unseen traffic with an 18.6% performance improvement. The results confirm that TrafficLLM is easy to scale and achieves accurate detection performance on enterprise traffic.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** none — Adjacent background (LLM/agents for security operations or network security); no direct overlap with the engine's traffic+radio+SCMS joint simulation.
**Novelty implications:** Does not restrict any candidate claim.
