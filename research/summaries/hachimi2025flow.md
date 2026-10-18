# Flow-Rule Generation for SDN Using LLMs with Retry-Based Deployment Validation
Anouar El Hachimi, Nicola Di Cicco, Memedhe Ibrahimi, Francesco Musumeci et al. · 2025 21st International Conference on Network and Service Management (CNSM) 2025 · https://doi.org/10.23919/cnsm67658.2025.11297487 · relevance: medium · basis: abstract

**Problem:** Translating natural language into deployable SDN flow rules.
**Approach:** LLM pipeline with intent clarification and retry-based correction, YAML intermediate, OpenFlow JSON for Ryu.
**Tools and scale:** Ryu controller; tasks of varying complexity.
**Results:** Accuracy up to 96.7%; about $0.08 per 100 configurations.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** partial — LLM generates validated configuration, like our configuring agent, but for SDN.
**Novelty implications:** Retry-validated LLM configuration is established practice.
