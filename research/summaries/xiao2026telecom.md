# TeleCom-Bench: How Far Are Large Language Models from Industrial Telecommunication Applications?
Jieting Xiao, Yun Lin, Huizhen Qiu, Rui Ma et al. · arXiv 2026 · https://arxiv.org/abs/2605.18025 · relevance: medium · basis: abstract

**Problem:** While Large Language Models have achieved remarkable integration in various vertical scenarios, their deployment in the telecommunications domain remains exploratory due to the lack of a standardized evaluation framework. Current telecom benchmarks primarily focus on static, foundational knowledge and isolated atomic skills, neglecting the ...
**Approach:** To bridge this gap, we present TeleCom-Bench, a comprehensive benchmark comprising 12 evaluation sets with 22,678 curated samples, which evaluates LLMs across a synergistic hierarchy: (1) Multi-dimensional Knowledge Comprehension, which integrates telecommunication fundamentals, 3GPP protocols, and 5G network architecture with proprietary product knowledge across wired, core, and wireless ...
**Tools and scale:** Current telecom benchmarks primarily focus on static, foundational knowledge and isolated atomic skills, neglecting the equipment-specific documentation and end-to-end industrial workflows essential for real-world production systems.
**Results:** Evaluations of eight state-of-the-art LLMs reveal a universal Execution Wall: while models achieve 90% accuracy in linguistic interface tasks such as intent recognition and entity extraction, performance collapses to approximately 30% in procedural execution tasks like solution ... This capability gap demonstrates that current LLMs function competently as diagnosticians but fail as field engineers.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — LLM agents for network/security operations; our engine has an LLM agent that configures and analyses runs, but this work is not V2X and not tied to a deterministic simulator with ground truth.
**Novelty implications:** LLM agents for SOC/NetOps/RCA are established; cross-layer V2X root-cause analysis against simulator ground truth is not covered here.
