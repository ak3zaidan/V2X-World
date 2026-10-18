# Firewalls to Secure Dynamic LLM Agentic Networks
Sahar Abdelnabi, Amr Gomaa, Eugene Bagdasarian, P. O. Kristensson et al. · Trans. Mach. Learn. Res. 2025 · https://arxiv.org/abs/2502.01822 · relevance: medium · basis: abstract

**Problem:** The emergence of agent-to-agent communication protocols mirrors the early internet: powerful connectivity with minimal security infrastructure. When AI agents communicate on behalf of users, every message crosses a trust boundary where the user's personal data and the external agent's unconstrained language each present distinct risks.
**Approach:** We address both through a dual-firewall architecture grounded in a unifying principle: each task defines a context, and both sides of the communication carry information far exceeding what that context requires.
**Tools and scale:** Mentioned in abstract: LLM, GPT, field. (From the abstract only.)
**Results:** Both firewalls operate in a trusted environment isolated from external input, applying domain-specific rules learned automatically from demonstrations. Across 864 attacks spanning three domains on the ConVerse benchmark, our architecture reduces privacy attack success rates (e.g., from 84% to 10% for GPT-5) and security attacks (from 60% to 3%), while maintaining or even improving task completion quality.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** none — Adjacent background (LLM/agents for security operations or network security); no direct overlap with the engine's traffic+radio+SCMS joint simulation.
**Novelty implications:** Does not restrict any candidate claim.
