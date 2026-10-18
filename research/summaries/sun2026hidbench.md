# HIDBench: Benchmarking Large Language Models for Host-Based Intrusion Detection
Danyu Sun, Jinghuai Zhang, Yuan Tian, Zhou Li · arXiv 2026 · https://arxiv.org/abs/2605.21773 · relevance: medium · basis: abstract

**Problem:** Recent benchmark efforts have advanced the evaluation of large language models (LLMs) in cybersecurity, including tasks such as penetration testing and vulnerability identification. However, a critical cybersecurity task, namely intrusion detection from system logs, remains unexplored.
**Approach:** In this work, we present a new benchmark to assess LLMs' capabilities in supporting host-based intrusion detection systems (HIDS).
**Tools and scale:** Recent benchmark efforts have advanced the evaluation of large language models (LLMs) in cybersecurity, including tasks such as penetration testing and vulnerability identification.
**Results:** Our benchmark unifies three public system log datasets, DARPA-E3, DARPA-E5, and NodLink, and introduces a data construction pipeline that transforms raw host telemetry into LLM-compatible inputs, enabling systematic evaluation under realistic intrusion detection settings. While many models achieve high precision (often above 0.8) on simpler datasets, their performance degrades significantly as system logs become noisier and more complex, with MCC frequently dropping below 0.5 and false positive rates increasing sharply.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — LLM agents for network/security operations; our engine has an LLM agent that configures and analyses runs, but this work is not V2X and not tied to a deterministic simulator with ground truth.
**Novelty implications:** LLM agents for SOC/NetOps/RCA are established; cross-layer V2X root-cause analysis against simulator ground truth is not covered here.
