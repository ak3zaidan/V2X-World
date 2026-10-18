# LLM-Assisted Design and Analytics of Next-Generation Optical Transport Networks
Imran Chowdhury Dipto, Sanwal Zeb, M. Masood, I. Khan et al. · IEEE Conference on Network Softwarization 2026 · https://doi.org/10.1109/netsoft70012.2026.11603451 · relevance: medium · basis: abstract

**Problem:** Optical networks are driving an increasing demand for network automation and intent-driven design, which requires translating high-level operator requests into network actions. In this paper, we present an LLM-based system for design and analytics of optical networks, where LLMs play the key role as reasoning agents, managing network analytics.
**Approach:** In this paper, we present an LLM-based system for design and analytics of optical networks, where LLMs play the key role as reasoning agents, managing network analytics.
**Tools and scale:** The proposed framework integrates network queries with domain specific Python modules for topology generation, graph analytics, and equipment-aware referencing.
**Results:** Four open-source LLMs have been evaluated, namely Qwen-2.5-14B-Instruct, LLaMA-3.1-8B, Mistral-NeMo-12B, and Gemma-2-9B, using a benchmark of representative network engineering queries ranging in varying network tasks. The results show that the Qwen-2.5-14B-Instruct model achieved the highest overall accuracy of 96.97 % with the lowest hallucination, while smaller models such as LLaMA and Gemma delivered competitive performance.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — LLM agents for network/security operations; our engine has an LLM agent that configures and analyses runs, but this work is not V2X and not tied to a deterministic simulator with ground truth.
**Novelty implications:** LLM agents for SOC/NetOps/RCA are established; cross-layer V2X root-cause analysis against simulator ground truth is not covered here.
