# WirelessBench: A Tolerance-Aware LLM Agent Benchmark for Wireless Network Intelligence
Jing-Wen Tong, Fang Liu, Linkai Xv, Shi-Liang Lu et al. · arXiv.org 2026 · https://arxiv.org/abs/2603.21251 · relevance: medium · basis: abstract

**Problem:** LLM agents are emerging as a key enabler for autonomous wireless network management. Reliably deploying them, however, demands benchmarks that reflect real engineering risk.
**Approach:** We present \wb{}, the first tolerance-aware, tool-integrated benchmark for LLM-based wireless agents.
**Tools and scale:** Mentioned in abstract: LLM, GPT. (From the abstract only.)
**Results:** \wb{} is organized as a three-tier cognitive hierarchy: domain knowledge reasoning (WCHW, 1{,}392 items), intent-driven resource allocation (WCNS, 1{,}000 items), and proactive multi-step decisions under mobility (WCMSA, 1{,}000 items). Moreover, \wb{} is established on three design principles: \emph{(i)}~tolerance-aware scoring with catastrophic-error detection; \emph{(ii)}~tool-necessary tasks requiring a 3GPP-compliant ray-tracing query for channel quality; and \emph{(iii)}~Chain-of-Thought (CoT)-traceable items, where every benchmark item ships with a complete CoT trajectory enabling fine-grained diagnosis of where in the reasoning chain an agent fails. Our numerical results show that the direct-prompting model (GPT-4o) scores $68\%$, trailing a tool-integrated agent ($84.64\%$) by $16.64$\,pp; $23\%$ of errors are catastrophic failures invisible to exact-match metrics.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** partial — Tolerance-aware tool-integrated benchmark for wireless LLM agents.
**Novelty implications:** Agent benchmarks exist for wireless; none for V2X security/PKI.
