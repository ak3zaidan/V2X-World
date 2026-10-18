# NetInjectBench: Benchmarking Indirect Prompt Injection in Tool-Using Large Language Model Agents for Network Operations
Ruksat Khan Shayoni, Muhammad Faraz Shoaib, S M Asif Hossain, M. F. Mridha · arXiv 2026 · https://arxiv.org/abs/2607.10490 · relevance: medium · basis: abstract

**Problem:** Tool-using large language model (LLM) agents are attractive for network operations, but tickets, alerts, logs, runbooks, and ChatOps messages can carry indirect prompt injections. We present NetInjectBench, a 130-scenario benchmark that separates untrusted artifact text, trusted policy metadata, and evaluation labels for network-operation tool use.
**Approach:** The sample contains 40 benign, 40 weak-attack, 40 strong-attack, and 10 approved high-impact change scenarios; each is evaluated with Qwen2.5-7B, Llama3.1-8B, and Mistral-7B.
**Tools and scale:** Mentioned in abstract: LLM. (From the abstract only.)
**Results:** Across 240 attack instances, naive execution reached an 82.50% unsafe tool-action rate. Prompt-only safety, Self-Reminder, Spotlighting, and a Two-Pass LLM Judge reduced this rate to 25.63%, 21.67%, 18.33%, and 10.00%, respectively. Static allowlisting reached 5.00% but blocked all approved changes, yielding 0.00% usefulness and 100.00% overblocking on approved cases.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** partial — Prompt-injection risk for tool-using network-ops agents.
**Novelty implications:** Relevant risk for our LLM agent reading run artefacts.
