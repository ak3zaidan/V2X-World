# Machine Learning for QoS Prediction in Vehicular Communication: Challenges and Solution Approaches
Alexandros Palaios, Christian L. Vielhaus, Daniel F. Külzer, Cara Watermann, Rodrigo Hernangomez, Sanket Partani et al. · IEEE Access 2023 · https://doi.org/10.1109/access.2023.3303528 · relevance: high · basis: full text (extracted sections)

**Problem:** ML-based predictive QoS for vehicles needs reliable methodology.
**Approach:** Studies the ML workflow for max-throughput prediction: sampling, dataset characteristics, splits, data availability, feature engineering.
**Tools and scale:** Real collected vehicular cellular data (details in paper).
**Results:** Random splits may overestimate performance by more than twofold.
**Limitations and future work (as stated by the authors):**
- Radio environment diversity (urban vs rural, deployment) limits model transfer.
**Overlap with our engine:** partial - methodological warning relevant to any learned surrogate we train (direction 3).
**Novelty implications:** Cautions that our benchmark should use non-random (spatial/temporal) splits.
