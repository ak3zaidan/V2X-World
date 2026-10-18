# Enhancing Anomaly Detection in Autonomous Systems with Explainable AI
Alper Alan, Mustafa Kutlu, Ruba Skaik et al. · Journal of Physics Conference Series 2026 · https://doi.org/10.1088/1742-6596/3191/1/012013 · relevance: medium · basis: abstract

**Problem:** Abstract The increasing deployment of Vehicle-to-Everything (V2X) communication in connected and autonomous vehicles has heightened concerns over cybersecurity and the need for transparent intrusion detection systems in safety-critical Intelligent Transportation Systems (ITS).
**Approach:** This study introduces an interpretable misbehavior detection framework that combines the XGBoost classifier with SHapley Additive exPlanations (SHAP) to address these challenges.
**Tools and scale:** Using the VEREMI dataset, which encompasses various attack scenarios in vehicular environments, the model was optimized through stratified sampling, feature encoding, regularization, and parameter tuning, resulting in a ROC-AUC of 0.6644 and an average precision of 0.6101. Performance was validated on a large-scale dataset exceeding 4.4 million samples, collected from VANET simulations under diver
**Results:** No quantitative results stated in the abstract.
**Limitations and future work (as stated by the authors):**
- "This study introduces an interpretable misbehavior detection framework that combines the XGBoost classifier with SHapley Additive exPlanations (SHAP) to address these challenges."
**Overlap with our engine:** partial — addresses misbehaviour/attack detection (direction 1 area); datasets/twins/surrogates (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
