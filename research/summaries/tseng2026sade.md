# SADE: Symptom-Aware Diagnostic Escalation for LLM-Based Network Troubleshooting
Kuan-Hao Tseng, Niruth Bogahawatta, Yasod Ginige, Kosta Dekic et al. · arXiv 2026 · https://arxiv.org/abs/2605.04530 · relevance: medium · basis: abstract

**Problem:** LLM agents for network troubleshooting localise root causes poorly.
**Approach:** SADE: phase-gated diagnostic workflow encoding Cisco methodology with fault-family skills.
**Tools and scale:** NIKA benchmark; 523 held-out incidents, eleven unseen scenarios.
**Results:** Root-cause F1 +37 points over ReAct + GPT-5; 22 points attributed to the policy alone.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** partial — LLM agent root-cause analysis, as in our runner-up direction, but for IP networks.
**Novelty implications:** LLM root-cause benchmarks exist for IP networks; a cross-layer V2X/PKI root-cause benchmark remains open.
