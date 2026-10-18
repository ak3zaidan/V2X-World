# CAIP: Detecting Router Misconfigurations with Context-Aware Iterative Prompting of LLMs
Xi Jiang, Aaron Gember, Nick Feamster · arXiv.org 2024 · https://arxiv.org/abs/2411.14283 · relevance: medium · basis: abstract

**Problem:** Model checkers and consistency checkers detect critical errors in router configurations, but these tools require significant manual effort to develop and maintain. LLM-based Q&A models have emerged as a promising alternative, allowing users to query partitions of configurations through prompts and receive answers based on learned patterns, thanks to transformer models pre-trained on vast datasets that provide generic configuration context for interpreting router configurations.
**Approach:** We introduce a Context-Aware Iterative Prompting (CAIP) framework that automates network-specific context extraction and optimizes LLM prompts for more precise router misconfiguration detection.
**Tools and scale:** Mentioned in abstract: LLM, real-world. (From the abstract only.)
**Results:** CAIP addresses three challenges: (1) efficiently mining relevant context from complex configuration files, (2) accurately distinguishing between pre-defined and user-defined parameter values to prevent irrelevant context from being introduced, and (3) managing prompt context overload with iterative, guided interactions with the model. Our evaluations on synthetic and real-world configurations show that CAIP improves misconfiguration detection accuracy by more than 30% compared to partition-based LLM approaches, model checkers, and consistency checkers, uncovering over 20 previously undetected misconfigurations in real-world configurations.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** none — Adjacent background (LLM agents for network configuration/operations); no direct overlap with the engine's traffic+radio+SCMS joint simulation.
**Novelty implications:** Does not restrict any candidate claim.
