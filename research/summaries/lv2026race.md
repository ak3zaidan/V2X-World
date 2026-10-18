# RACE-SOC: Risk-Constrained Auditable Coordination Evaluation for LLM-Based Multi-Agent Security Operations
Yi Lv, Cheng Wang · Computers, Materials &amp; Continua 2026 · https://doi.org/10.32604/cmc.2026.088835 · relevance: medium · basis: abstract

**Problem:** Large language model (LLM) agents are now being used in security operations center (SOC) workflows to summarise security alerts, support triage, and suggest escalation or containment measures. Existing evaluations tend to focus on answer quality or benchmark accuracy, while the operational risks of multi-agent coordination are not clearly exposed.
**Approach:** This paper proposes RACE-SOC, a Risk-constrained Auditable Coordination Evaluation protocol for LLM-based multi-agent SOC triage.
**Tools and scale:** Existing evaluations tend to focus on answer quality or benchmark accuracy, while the operational risks of multi-agent coordination are not clearly exposed.
**Results:** We also add a matched same-temperature repeated-inference control at K = 5 and K = 10 to separate repeated sampling from the role-conditioned pipeline. The control obtains a higher mean net score in five of six model–K strata, while the role-conditioned pipeline is higher for GLM-5.2 at K = 10.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — LLM agents for network/security operations; our engine has an LLM agent that configures and analyses runs, but this work is not V2X and not tied to a deterministic simulator with ground truth.
**Novelty implications:** LLM agents for SOC/NetOps/RCA are established; cross-layer V2X root-cause analysis against simulator ground truth is not covered here.
