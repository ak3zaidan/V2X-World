# LanG -- A Governance-Aware Agentic AI Platform for Unified Security Operations
Anes Abdennebi, Nadjia Kara, Laaziz Lahlou, Hakima Ould-Slimane · arXiv 2026 · https://arxiv.org/abs/2604.05440 · relevance: medium · basis: abstract

**Problem:** Modern Security Operations Centers struggle with alert fatigue, fragmented tooling, and limited cross-source event correlation. Challenges that current Security Information Event Management and Extended Detection and Response systems only partially address through fragmented tools.
**Approach:** This paper presents the LLM-assisted network Governance (LanG), an open-source, governance-aware agentic AI platform for unified security operations contributing: (i) a Unified Incident Context Record with a correlation engine (F1 = 87%), (ii) an Agentic AI Orchestrator on LangGraph with human-in-the-loop checkpoints, (iii) an LLM-based Rule Generator finetuned on four base models producing ...
**Tools and scale:** Not stated in the abstract.
**Results:** Finetuned anomaly and threat detectors achieve weighted F1 scores of 99.0% and 91.0%, respectively, in intrusion-detection benchmarks, running inferences in $\approx$21 ms with a machine-side mean time to detect of 1.58 s, and the rule generator exceeds 91% deployability on live ...
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — LLM agents for network/security operations; our engine has an LLM agent that configures and analyses runs, but this work is not V2X and not tied to a deterministic simulator with ground truth.
**Novelty implications:** LLM agents for SOC/NetOps/RCA are established; cross-layer V2X root-cause analysis against simulator ground truth is not covered here.
