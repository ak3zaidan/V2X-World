# MeshAgent: Enabling Reliable Network Management with Large Language Models
Yajie Zhou, Kevin Hsieh, Sathiya Kumaran Mani, Srikanth Kandula et al. · Proceedings of the ACM on Measurement and Analysis of Computing Systems 2025 · https://doi.org/10.1145/3771567 · relevance: medium · basis: abstract

**Problem:** The emergence of large language models (LLMs) offers great promise for building domain-specific agents, but adapting them for network management remains challenging. To understand why, we conduct a case study on network management tasks and find that state-of-the-art specialization techniques rely heavily on extensive, high-quality task-specific data to produce precise solutions.
**Approach:** Motivated by this gap, we propose MeshAgent, a new workflow that improves precision by extracting domain-specific invariants from sample queries and encoding them as constraints. We evaluate our method across three network management applications and a user study involving industrial network professionals, showing that it complements existing techniques and consistently improves accuracy.
**Tools and scale:** Mentioned in abstract: LLM, real-world. (From the abstract only.)
**Results:** We also introduce reliability metrics and demonstrate that our system is more dependable, with the ability to abstain when confidence is low. Overall, our results show that MeshAgent achieves over 95% accuracy, reaching 100% when paired with fine-tuned agents, and improves accuracy by up to 26% compared to baseline methods.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** none — Adjacent background (LLM agents for network configuration/operations); no direct overlap with the engine's traffic+radio+SCMS joint simulation.
**Novelty implications:** Does not restrict any candidate claim.
