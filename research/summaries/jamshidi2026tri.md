# Tri-LLM Cooperative Federated Zero-Shot Intrusion Detection with Semantic Disagreement and Trust-Aware Aggregation
Saeid Jamshidi, Omar Abdul Wahab, Foutse Khomh, Kawser Wazed Nafi · arXiv 2026 · https://arxiv.org/abs/2602.00219 · relevance: medium · basis: abstract

**Problem:** Federated learning (FL) has become an effective paradigm for privacy-preserving, distributed Intrusion Detection Systems (IDS) in cyber-physical and Internet of Things (IoT) networks, where centralized data aggregation is often infeasible due to privacy and bandwidth constraints. Despite its advantages, most existing FL-based IDS assume closed-set learning ...
**Approach:** This paper introduces a semantics-driven federated IDS framework that incorporates language-derived semantic supervision into federated optimization, enabling open-set and zero-shot intrusion detection for previously unseen attack behaviors.
**Tools and scale:** Not stated in the abstract.
**Results:** The approach constructs semantic attack prototypes using a Tri-LLM ensemble of GPT-4o, DeepSeek-V3, and LLaMA-3-8B, aligning distributed telemetry features with high-level attack concepts. The framework achieves over 80% zero-shot detection accuracy on unseen attack patterns, improving zero-day discrimination by more than 10% compared to similarity-based baselines, while maintaining low aggregation instability in the presence of unreliable or compromised clients.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — LLM agents for network/security operations; our engine has an LLM agent that configures and analyses runs, but this work is not V2X and not tied to a deterministic simulator with ground truth.
**Novelty implications:** LLM agents for SOC/NetOps/RCA are established; cross-layer V2X root-cause analysis against simulator ground truth is not covered here.
