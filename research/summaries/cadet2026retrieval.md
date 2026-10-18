# Retrieval-Augmented LLMs for Security Incident Analysis
Xavier Cadet, Aditya Vikram Singh, Harsh Mamania, Edward Koh et al. · arXiv 2026 · https://arxiv.org/abs/2603.18196 · relevance: medium · basis: abstract

**Problem:** Investigating cybersecurity incidents requires collecting and analyzing evidence from multiple log sources, including intrusion detection alerts, network traffic records, and authentication events. This process is labor-intensive: analysts must sift through large volumes of data to identify relevant indicators and piece together what happened.
**Approach:** We present a RAG-based system that performs security incident analysis through targeted query-based filtering and LLM semantic reasoning.
**Tools and scale:** Not stated in the abstract.
**Results:** Attack step detection on the Active Directory scenario reaches 100% precision and up to 96% recall with an enumeration prompt. These results demonstrate that combining targeted query-based filtering with RAG-based retrieval -- confirmed essential by ablation studies -- enables accurate, cost-effective security analysis within LLM context limits.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — LLM agents for network/security operations; our engine has an LLM agent that configures and analyses runs, but this work is not V2X and not tied to a deterministic simulator with ground truth.
**Novelty implications:** LLM agents for SOC/NetOps/RCA are established; cross-layer V2X root-cause analysis against simulator ground truth is not covered here.
