# Covert Prompt Transmission for Secure Large Language Model Services
Ruichen Zhang, Yinqiu Liu, Shunpu Tang et al. · arXiv 2025 · https://arxiv.org/abs/2504.21311 · relevance: low · basis: abstract

**Problem:** This paper investigates covert prompt transmission for secure and efficient large language model (LLM) services over wireless networks.
**Approach:** To solve this problem, we first propose a prompt compression and encryption (PCAE) framework, performing surprisal-guided compression followed by lightweight permutation-based encryption. Specifically, PCAE employs a locally deployed small language model (SLM) to estimate token-level surprisal scores, selectively retaining semantically critical tokens while discarding redundant ones.
**Tools and scale:** Mentions: reinforcement learning.
**Results:** This significantly reduces computational overhead and transmission duration. To further enhance covert wireless transmission, we then develop a group-based proximal policy optimization (GPPO) method that samples multiple candidate actions for each state, selecting the optimal one within each group and incorporating a Kullback-Leibler (KL) divergence penalty to improve policy stability and exploration.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
