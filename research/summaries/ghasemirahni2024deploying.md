# Deploying Stateful Network Functions Efficiently using Large Language Models
Hamid Ghasemirahni, Alireza Farshin, Mariano Scazzariello, Marco Chiesa, Dejan Kostić · Proceedings of the 4th Workshop on Machine Learning and Systems 2024 · https://doi.org/10.1145/3642970.3655836 · relevance: low · basis: abstract

**Problem:** Stateful network functions are increasingly used in data centers.
**Approach:** However, their scalability remains a significant challenge since parallelizing packet processing across multiple cores requires careful configuration t o avoid compromising the application's semantics or performance. This challenge is particularly important when deploying multiple stateful functions on multi-core servers.
**Tools and scale:** Not stated in the abstract.
**Results:** Our evaluation shows that, utilizing GPT-4, FlowMage is able to find and apply optimized configuration when deploying stateful NFs chain on a server, resulting in significant p erformance improvement (up to 11×) in comparison to the default configuration of the system.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — touches LLM agents but not the joint traffic/radio/SCMS closed loop.
**Novelty implications:** Nothing in our candidate directions is ruled out; useful only as background on LLM agents.
