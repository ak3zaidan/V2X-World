# From Stealthy Data Fabrication to Unsafe Driving: Realistic Scenario Attacks on Collaborative Perception
Qingzhao Zhang, Runting Zhang, Z. Morley Mao · arXiv 2026 · https://arxiv.org/abs/2605.01301 · relevance: high · basis: full text

**Problem:** Whether collaborative-perception data fabrication can cause safety-critical outcomes in realistic dynamic scenes.
**Approach:** Stealthy pose manipulation of existing objects below detection thresholds, propagated through tracking and prediction; online scenario-aware observe-predict-plan-execute attack; object-level defence.
**Tools and scale:** OPV2V and V2X-Real datasets; CARLA/Autoware ecosystem; white-box and query-based optimisation.
**Results:** Attack induces unsafe driving while evading anomaly detection; perturbations transfer across predictors (exact rates in paper).
**Limitations and future work (as stated by the authors):**
- Authors argue end-to-end security evaluation is needed beyond perception-level defences.
**Overlap with our engine:** partial — attack to unsafe-driving outcome via end-to-end effects; perception layer, not PKI.
**Novelty implications:** End-to-end attack-to-safety evaluation exists for CP; tying it to MA detection and revocation remains open.
