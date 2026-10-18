# Large Language Models (LLMs) for Telecom Root Cause Analysis (RCA): A Structured Reasoning Framework for Evidence-Grounded Diagnosis
Hao Zhou, Mandar Kulkarni, Hao Chen, Yan Xin et al. · arXiv 2026 · https://arxiv.org/abs/2609.02805 · relevance: medium · basis: abstract

**Problem:** Root cause analysis (RCA) is a critical task in telecom network operations, but diagnosing performance degradations in modern 5G and emerging 6G networks remains challenging due to complex cross-layer dependencies. While large language models (LLMs) offer promising capabilities for reasoning and knowledge integration, directly applying vanilla LLMs to ...
**Approach:** This work first reviews the evolution of telecom RCA from rule-based and machine learning (ML) approaches to emerging LLM-enabled techniques, and provides an overview of recent paradigms, including structured reasoning, retrieval-augmented knowledge grounding, agentic orchestration, and verifiable reasoning.
**Tools and scale:** These cross-dataset results highlight the importance of structured reasoning design for practical LLM-based RCA systems in next-generation telecom networks.
**Results:** Experimental results on two 5G RCA datasets, TeleLogs and TelecomTS, demonstrate that the proposed framework consistently improves diagnostic accuracy and decision consistency compared with baseline techniques.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — LLM agents for network/security operations; our engine has an LLM agent that configures and analyses runs, but this work is not V2X and not tied to a deterministic simulator with ground truth.
**Novelty implications:** LLM agents for SOC/NetOps/RCA are established; cross-layer V2X root-cause analysis against simulator ground truth is not covered here.
