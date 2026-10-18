# How Reliable Are Explanations? Sender-Disjoint Evaluation of Explainable Misbehavior Detection in VANETs
Ruba Skaik, Alper Alan, Mustafa Kutlu, Mishri Almarshoud et al. · SSRN Electronic Journal 2026 · https://doi.org/10.2139/ssrn.7411187 · relevance: high · basis: abstract

**Problem:** VeReMi studies leak sender identity and judge explanations visually.
**Approach:** Sender-disjoint protocol, 48-feature schema, information-parity ablation, quantified SHAP/LIME reliability.
**Tools and scale:** VeReMi Extension, 3.19 million BSMs, 20 classes, seven classifiers.
**Results:** LightGBM per-message weighted F1 0.930; sender-level EWMA aggregation raises vehicle-level F1 to 0.988.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — benchmark hygiene relevant to direction 3.
**Novelty implications:** Sender-level leakage is documented; our benchmark must use sender-disjoint splits.
