# From Investigation Failures to Reliable SOC Agents: Understanding and Improving LLM-Based Alert Triage
Saimon Amanuel Tsegai, Alex Kantchelian,  Danfeng,  Yao et al. · arXiv 2026 · https://arxiv.org/abs/2610.10608 · relevance: medium · basis: abstract

**Problem:** Security operations centers (SOCs) must triage large volumes of alerts, most of which are benign, while missed attacks can remain uninvestigated. Tool-using large language model (LLM) agents can retrieve evidence during triage, but it remains unclear how reasoning strategies determine what to gather and when an investigation is sufficient to close an alert.
**Approach:** We study five representative approaches spanning single-pass tool use, iterative retrieval, sampled investigations, self-review, and explicit verification.
**Tools and scale:** To support this study, we build ALERT-BENCH, an interactive benchmark that replays enterprise telemetry through a live SIEM and requires each system to retrieve evidence.
**Results:** On the same alerts, AIDA achieves an F1 score of 0.958, compared with 0.371-0.744 for the studied approaches, and reduces the false-negative rate from 40.4% to 3.1% while escalating 18.4% of alerts to analysts. These results show that structuring evidence retrieval and decision review can substantially improve agentic SOC triage.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — LLM agents for network/security operations; our engine has an LLM agent that configures and analyses runs, but this work is not V2X and not tied to a deterministic simulator with ground truth.
**Novelty implications:** LLM agents for SOC/NetOps/RCA are established; cross-layer V2X root-cause analysis against simulator ground truth is not covered here.
