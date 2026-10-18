# Policy-Guided Threat Hunting: An LLM enabled Framework with Splunk SOC Triage
Rishikesh Sahay, Bell Eapen, Weizhi Meng, Md Rasel Al Mamun et al. · arXiv 2026 · https://arxiv.org/abs/2603.23966 · relevance: medium · basis: abstract

**Problem:** With frequently evolving Advanced Persistent Threats (APTs) in cyberspace, traditional security solutions approaches have become inadequate for threat hunting for organizations. Moreover, SOC (Security Operation Centers) analysts are often overwhelmed and struggle to analyze the huge volume of logs received from diverse devices in organizations.
**Approach:** To address these challenges, we propose an automated and dynamic threat hunting framework for monitoring evolving threats, adapting to changing network conditions, and performing risk-based prioritization for the mitigation of suspicious and malicious traffic.
**Tools and scale:** We evaluated the framework against a publicly available benchmark dataset, as well as against a simulated dataset.
**Results:** The experimental results show that the framework can effectively adapt to different SOC objectives autonomously and identify suspicious and malicious traffic.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — LLM agents for network/security operations; our engine has an LLM agent that configures and analyses runs, but this work is not V2X and not tied to a deterministic simulator with ground truth.
**Novelty implications:** LLM agents for SOC/NetOps/RCA are established; cross-layer V2X root-cause analysis against simulator ground truth is not covered here.
