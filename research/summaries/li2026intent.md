# Intent Interpretation at RIC Timescales: Jev Decision Models versus Large Language Models in 6G Open RAN
Delong Li, Xu Wang, Haochen Gong, Rui Lang, Guangsheng Yu · arXiv 2026 · https://arxiv.org/abs/2609.23136 · relevance: low · basis: abstract

**Problem:** Intent-based Open RAN needs an interpreter that turns intents into A1 policies within the loop of the RAN intelligent controller (RIC).
**Approach:** Decision models such as Jev-1.13.0 return typed policy fields, whereas generative large language models (LLMs) produce the policy token by token. We ask whether the extra delay of LLMs costs control deadlines, RIC capacity, or radio performance.
**Tools and scale:** Mentioned in abstract: ns-3, simulation.
**Results:** Median interpretation takes 0.286 to 2.35 s, against under 25 ms for A1 and E2 transfer. Jev-1.13.0 meets the 1 s near-real-time budget on 99.8% of calls, while two hosted LLMs meet it on 17.9% and 0%.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — touches network/traffic simulation, LLM agents but not the joint traffic/radio/SCMS closed loop.
**Novelty implications:** Nothing in our candidate directions is ruled out; useful only as background on network/traffic simulation.
