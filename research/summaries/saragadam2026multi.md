# Multi-Agent Retrieval-Augmented Generation for Efficient Cloud Knowledge Base Search in Telecom SNOC Environment
Harish Saragadam, Sudhanshu Sharma, Ipsha Routray · arXiv 2026 · https://arxiv.org/abs/2609.01618 · relevance: medium · basis: abstract

**Problem:** Telecom Service and Network Operations Centers (SNOCs) rely on large collections of cloud documents, including Standard Operating Procedures (SOPs), vendor technical manuals, incident reports, and configuration guides, to maintain uninterrupted network operations. During critical incidents, engineers must quickly retrieve accurate information, yet ...
**Approach:** This paper presents Athena for Cloud Knowledge Base, a fully offline, multi agent Retrieval Augmented Generation (RAG) framework designed for enterprise cloud document search in Vodafone Idea's SNOC environment.
**Tools and scale:** Not stated in the abstract.
**Results:** To improve answer reliability, the framework performs per chunk LLM evaluation with explicit attribution verification, assessing each MMR selected chunk independently before generating a response. Experiments on a corpus of 4200 SNOC cloud documents containing 312000 indexed chunks show that the proposed approach achieves an MRR at 10 of 0.910 and an Exact Match (EM) score of 78.4 percent, outperforming single stage dense retrieval by 14.6 percentage points.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — LLM agents for network/security operations; our engine has an LLM agent that configures and analyses runs, but this work is not V2X and not tied to a deterministic simulator with ground truth.
**Novelty implications:** LLM agents for SOC/NetOps/RCA are established; cross-layer V2X root-cause analysis against simulator ground truth is not covered here.
