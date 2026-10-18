# SimSkill: A Self-Evolving LLM Agent for Skill and Knowledge Accumulation in Traffic Simulation
Qi Liu, Qin-Zheng Wang, Can Li, Y. Bie et al. ·  2026 · https://arxiv.org/abs/2609.03753 · relevance: medium · basis: abstract

**Problem:** LLM agents do not accumulate skills for traffic simulation.
**Approach:** SimSkill self-evolving agent around SUMO with action-critic verification and memories.
**Tools and scale:** SUMO; two held-out benchmarks, three LLMs.
**Results:** Verified success up to +25 points; benefits backbone- and budget-dependent.
**Limitations and future work (as stated by the authors):**
- Memory does not improve every model or uniformly reduce inference cost.
**Overlap with our engine:** partial — LLM agent operating a traffic simulator, akin to our configuring agent (traffic only).
**Novelty implications:** Agents driving SUMO exist; agents over a joint traffic-radio-PKI engine remain open.
