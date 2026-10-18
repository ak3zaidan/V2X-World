# NetConfEval: Can LLMs Facilitate Network Configuration?
Changjie Wang, Mariano Scazzariello, Alireza Farshin, Simone Ferlin, Dejan Kostić, Marco Chiesa · Proceedings of the ACM on Networking 2024 · https://doi.org/10.1145/3656296 · relevance: high · basis: abstract

**Problem:** Network configuration is error-prone; can LLMs translate natural-language policies into configurations?
**Approach:** NetConfEval benchmark with four tasks (formal spec generation, API call generation, routing algorithm development, low-level config generation); design principles; two GPT-4 prototypes (P4 configuration, integration with network synthesisers).
**Tools and scale:** LLM benchmarks; P4 devices.
**Results:** Comparative model results (not quantified in abstract).
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial - LLM translating natural-language requirements into configurations, similar to our LLM agent configuring ~300 settings, but for routers not a simulator.
**Novelty implications:** We cannot claim LLM-based network configuration from natural language is new; LLM-configured V2X simulation runs remain open.
