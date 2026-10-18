# GALA: Graph-Augmented LLM Agents for Root Cause Analysis and Incident Response in Microservices
Yifang Tian, Yaming Liu, Zichun Chong, Zihang Huang et al. · arXiv 2026 · https://arxiv.org/abs/2608.08968 · relevance: medium · basis: abstract

**Problem:** Microservice root cause analysis (RCA) requires correlating failures across heterogeneous telemetry within complex service dependency graphs. Existing methods often rely on a single telemetry modality; recent LLM-based approaches can suffer from unconstrained exploration and hallucination; and most systems stop at fault ranking without producing actionable ...
**Approach:** We present GALA+, a graph-augmented LLM agentic framework centered on graph-guided investigation, which uses service dependencies to bound exploration and refine diagnosis through localized multi-modal evidence.
**Tools and scale:** Not stated in the abstract.
**Results:** On two microservice benchmarks, GALA+ consistently achieves the strongest overall results, surpassing the best LLM-based baseline by more than 25 percentage points in AC@1, while also receiving the highest ratings from both SURE-Score and independent human SRE evaluation.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — Learned wireless/PHY model (foundation or world model); no V2X security, PKI or closed-loop traffic.
**Novelty implications:** Shows learned radio surrogates are active work; a surrogate trained on joint traffic-radio-PKI ground truth with counterfactual labels is not addressed.
