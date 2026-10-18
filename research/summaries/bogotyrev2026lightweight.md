# Lightweight Machine Learning-Based Intrusion Detection for Vehicle-to-Everything Communications
Roman Bogotyrev, F. Kamalov · ACM Symposium on Solid Modeling and Applications 2026 · https://doi.org/10.1109/sm69703.2026.11614159 · relevance: medium · basis: abstract

**Problem:** Data-falsification detection in V2X BSMs.
**Approach:** RF and XGBoost IDS with sender-based train/test split.
**Tools and scale:** VeReMi dataset.
**Results:** XGBoost 96.9% accuracy, 0.97 ROC-AUC, 2.48 MB model, sub-microsecond per-message inference.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — offline VeReMi MBD; ours runs detection in closed loop.
**Novelty implications:** Offline VeReMi classifiers are saturated; closed-loop evaluation remains open.
