# In-Context Autonomous Network Incident Response: An End-to-End Large Language Model Agent Approach
Yiran Gao, Kim Hammar, Tao Li · Proceedings of the AAAI Symposium Series 2026 · https://doi.org/10.1609/aaaiss.v9i1.42938 · relevance: high · basis: full text (extracted sections)

**Problem:** RL incident response needs handcrafted simulators and discards log semantics.
**Approach:** Single 14B LLM agent integrating perception, reasoning, planning (LLM-simulated outcomes) and action; in-context refinement of attack conjecture.
**Tools and scale:** LoRA fine-tuning of DeepSeek-14B on CSLE-IncidentResponse-V1 (50,000 pairs).
**Results:** F1 around 0.95-0.99 on perception; 23% shorter recovery time than frontier LLM baselines.
**Limitations and future work (as stated by the authors):**
- LLM-based simulation is time-consuming in complex environments (scalability).
- Evaluation procedures need improvement.
**Overlap with our engine:** partial - LLM security-operations agent; not V2X.
**Novelty implications:** LLM incident response exists in IT networks; V2X MA decision support remains open.
