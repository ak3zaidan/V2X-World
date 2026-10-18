# Penetration Testing of Agentic AI: A Comparative Security Analysis Across Models and Frameworks
Viet K. Nguyen, Mohammad I. Husain · arXiv 2025 · https://arxiv.org/abs/2512.14860 · relevance: low · basis: abstract

**Problem:** Agentic AI introduces security vulnerabilities that traditional LLM safeguards fail to address.
**Approach:** Our 130 total test cases reveal significant security disparities: AutoGen demonstrates a 52.3% refusal rate versus CrewAI's 30.8%, while model performance ranges from Nova Pro's 46.2% to Claude and Grok 2's 38.5%.
**Tools and scale:** Not stated in the abstract.
**Results:** Most critically, Grok 2 on CrewAI rejected only 2 of 13 attacks (15.4% refusal rate), and the overall refusal rate of 41.5% across all configurations indicates that more than half of malicious prompts succeeded despite enterprise-grade safety mechanisms.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Studies V2X attacks or their detection; overlaps the misbehaviour/attack modules but without linkage-based revocation, closed loop to the SCMS, or joint ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
