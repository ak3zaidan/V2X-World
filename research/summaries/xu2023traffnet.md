# TraffNet: Learning Causality of Traffic Generation for What-if Prediction
Ming Xu, Qiang Ai, Ruimin Li, Yunyi Ma et al. · arXiv 2023 · https://arxiv.org/abs/2303.15954 · relevance: medium · basis: abstract

**Problem:** Correlation-based traffic predictors cannot answer what-if questions.
**Approach:** TraffNet learns traffic generation mechanisms (OD demand, route assignment) on a heterogeneous road graph for what-if prediction.
**Tools and scale:** Synthetic datasets; code released.
**Results:** Effective on synthetic data (no figures in abstract).
**Limitations and future work (as stated by the authors):**
- Evaluated only on synthetic datasets.
**Overlap with our engine:** partial — learned what-if (counterfactual) traffic prediction, traffic side of direction 3; no radio or security.
**Novelty implications:** Causal what-if traffic surrogates exist; cross-layer V2X counterfactual surrogate remains open.
