# Insider Threat Detection and Evidence-Grounded Triage Using a Heterogeneous Dual-Branch Network and a Constrained LLM Agent
Kai Cheng, Dong-Kun Li, Wei-Dong Tang, Yi Shu et al. · Applied Sciences 2026 · https://doi.org/10.3390/app16147054 · relevance: medium · basis: abstract

**Problem:** Insider threats are difficult to detect and investigate because risky behavior can occur through valid accounts, develop slowly, and leave mixed behavioral traces. We study an analyst support architecture that links anomaly detection with evidence-based triage.
**Approach:** We study an analyst support architecture that links anomaly detection with evidence-based triage.
**Tools and scale:** On the CERT r4.2 dataset, the HDBN reaches 96.8% recall at the selected operating point.
**Results:** On the CERT r4.2 dataset, the HDBN reaches 96.8% recall at the selected operating point. In the constructed investigation tasks, the constrained ReAct agent reaches 91.8% end-to-end triage accuracy compared to an unconstrained LLM agent baseline under the same tool access setting.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — LLM agents for network/security operations; our engine has an LLM agent that configures and analyses runs, but this work is not V2X and not tied to a deterministic simulator with ground truth.
**Novelty implications:** LLM agents for SOC/NetOps/RCA are established; cross-layer V2X root-cause analysis against simulator ground truth is not covered here.
