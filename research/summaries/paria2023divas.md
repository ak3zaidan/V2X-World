# DIVAS: An LLM-based End-to-End Framework for SoC Security Analysis and Policy-based Protection
Sudipta Paria, Aritra Dasgupta, Swarup Bhunia · arXiv 2023 · https://arxiv.org/abs/2308.06932 · relevance: low · basis: abstract

**Problem:** Securing critical assets in a bus-based System-On-Chip (SoC) is imperative to mitigate potential vulnerabilities and prevent unauthorized access, ensuring the integrity, availability, and confidentiality of the system.
**Approach:** In this work, we propose DIVAS, a novel framework that leverages the knowledge base of LLMs to identify security vulnerabilities from user-defined SoC specifications, map them to the relevant Common Weakness Enumerations (CWEs), followed by the generation of equivalent assertions, and employ security measures through enforcement of security policies. The proposed framework is implemented using multiple ChatGPT and BARD models, and their performance was analyzed while generating relevant CWEs from the SoC specifications provided.
**Tools and scale:** Not stated in the abstract.
**Results:** The experimental results obtained from open-source SoC benchmarks demonstrate the efficacy of our proposed framework.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
