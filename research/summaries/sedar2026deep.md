# Deep Reinforcement Learning for Misbehavior Detection Under Partially Observable V2X Data
Roshan Sedar, Charalampos Kalalas · CoRR 2026 · https://arxiv.org/abs/2609.31217 · relevance: high · basis: full text (extracted sections)

**Problem:** MBD assumes fully observable V2X streams; missingness may be exploited.
**Approach:** DRL detection policies under incomplete observations; threat model of occlusion-based evasion and feature suppression.
**Tools and scale:** VeReMi; MCAR, MAR, MNAR patterns; XGBoost baseline.
**Results:** DRL beats XGBoost under natural partial observability but is highly vulnerable to evasion exploiting missingness; degrades more gradually under feature suppression.
**Limitations and future work (as stated by the authors):**
- DRL vulnerable to strategic missingness-based evasion.
**Overlap with our engine:** partial - MBD under realistic packet loss, which our engine produces natively from the radio stack; offline on VeReMi.
**Novelty implications:** Partial-observability MBD is studied offline; closed-loop evaluation with radio-induced missingness remains open.
