# FDE-Bench: Evaluating LLM Agents for Deployment Environment Configuration
Wei-Hang Ding, Jun-Fei Zhan, Yue-Ting Li, Qi-Rong Guo ·  2026 · https://arxiv.org/abs/2609.27571 · relevance: medium · basis: abstract

**Problem:** Evaluating LLM agents on deployment configuration.
**Approach:** FDE-Bench: 136 tasks across Docker, Compose, Kubernetes with programmatic gated checks.
**Tools and scale:** 2,145 checks; adversarial shortcut strategies.
**Results:** No adversarial strategy resolves any of 135 tasks; a vacuous health probe passes readiness.
**Limitations and future work (as stated by the authors):**
- Seven documented gaps in check annotations; need for downstream checks.
**Overlap with our engine:** none.
**Novelty implications:** No impact; methodology for checks that cannot be gamed is relevant.
