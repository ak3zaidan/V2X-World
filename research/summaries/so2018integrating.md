# Integrating Plausibility Checks and Machine Learning for Misbehavior Detection in VANET
Steven So, Prinkle Sharma, Jonathan Petit · 2018 17th IEEE International Conference on Machine Learning and Applications (ICMLA) 2018 · https://doi.org/10.1109/icmla.2018.00091 · relevance: high · basis: abstract

**Problem:** Detecting and classifying location spoofing misbehaviour in VANETs.
**Approach:** Uses plausibility check outputs as feature vectors for KNN and SVM classifiers.
**Tools and scale:** VeReMi dataset (offline).
**Results:** Detection precision improved by over 20% over the plausibility checks alone with recall within 5%; misbehaviour types can be classified.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial - ML misbehaviour detection, which our engine executes in closed loop; this work is offline on VeReMi.
**Novelty implications:** Plausibility-plus-ML MBD is not new; closed-loop impact on revocation and safety is not studied.
