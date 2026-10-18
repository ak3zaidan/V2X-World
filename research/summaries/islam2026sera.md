# SERA-IDS: Structured Experience Retrieval-Augmented Intrusion Detection with Small Language Models
Md Shamimul Islam, Ayesha S. Dina · arXiv 2026 · https://arxiv.org/abs/2610.03999 · relevance: medium · basis: abstract

**Problem:** Large language models (LLMs) offer a flexible approach to network intrusion detection, but direct classification of numerical flow records can be unreliable without traffic- specific decision boundaries. Retrieval-augmented generation (RAG) enables LLMs to leverage external experience; however, free-form textual experience requires the model to infer the ...
**Approach:** We propose SERA-IDS, a Structured Experience Retrieval-Augmented Intru- sion Detection framework that learns decision knowledge from classification errors.
**Tools and scale:** Not stated in the abstract.
**Results:** On the test sets, SERA-IDS improves macro F1 on NF-BoT- IoT from 11.66%, 9.26%, and 7.83% to 86.46%, 69.50%, and 83.51%, respectively, and on NF-ToN-IoT from 3.82%, 6.17%, and 1.94% to 84.16%, 87.66%, and 83.37%. These results show that structured experience retrieval enables small, free, and locally deployable SLMs to achieve strong intrusion-detection performance, including performance exceeding that of the re- ported existing work used in our comparison.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — LLM agents for network/security operations; our engine has an LLM agent that configures and analyses runs, but this work is not V2X and not tied to a deterministic simulator with ground truth.
**Novelty implications:** LLM agents for SOC/NetOps/RCA are established; cross-layer V2X root-cause analysis against simulator ground truth is not covered here.
