# MA-IDS: Multi-Agent RAG Framework for IoT Network Intrusion Detection with an Experience Library
Md Shamimul Islam, Luis G. Jaimes, Ayesha S. Dina · arXiv 2026 · https://arxiv.org/abs/2604.05458 · relevance: medium · basis: abstract

**Problem:** Network Intrusion Detection Systems (NIDS) face important limitations. Signature-based methods are effective for known attack patterns, but they struggle to detect zero-day attacks and often miss modified variants of previously known attacks, while many machine learning approaches offer limited interpretability.
**Approach:** To address these issues, we propose MA-IDS, a Multi-Agent Intrusion Detection System that combines Large Language Models (LLMs) with Retrieval Augmented Generation (RAG) for reasoning-driven intrusion detection.
**Tools and scale:** Evaluated on NF-BoT-IoT and NF-ToN-IoT benchmark datasets, MA-IDS achieves Macro F1-Scores of 89.75% and 85.22%, improving over zero-shot baselines of 17% and 4.96% by more than 72 and 80 percentage points.
**Results:** Evaluated on NF-BoT-IoT and NF-ToN-IoT benchmark datasets, MA-IDS achieves Macro F1-Scores of 89.75% and 85.22%, improving over zero-shot baselines of 17% and 4.96% by more than 72 and 80 percentage points. These results are competitive with SVM while providing rule-level explanations for every classification decision, demonstrating that retrieval-augmented reasoning offers a principled path toward explainable, self-improving intrusion detection for IoT networks.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — LLM agents for network/security operations; our engine has an LLM agent that configures and analyses runs, but this work is not V2X and not tied to a deterministic simulator with ground truth.
**Novelty implications:** LLM agents for SOC/NetOps/RCA are established; cross-layer V2X root-cause analysis against simulator ground truth is not covered here.
