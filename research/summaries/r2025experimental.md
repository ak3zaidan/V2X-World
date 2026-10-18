# Experimental Evaluation of GRU-Driven Deep Learning Model to Detect Anomalies in VANET Environment
Vinod Kumar R, S. V, V. P · 2025 International Conference on Emerging Research in Computational Science (ICERCS) 2025 · https://doi.org/10.1109/icercs65898.2025.11580458 · relevance: medium · basis: abstract

**Problem:** Vehicular Ad Hoc Networks (VANETs) form the backbone of next-generation intelligent transportation systems, enabling real-time communication between vehicles and roadside units.
**Approach:** In this work, a deep learning model that uses GRU to learn temporal relationships in vehicular communication patterns to detect and classify network anomalies with high accuracy is proposed. The model proposed reached maximum detection accuracy (96.4) than the baseline models such as LSTM (94.7), CNN (91.8), and Random Forest (89.5).
**Tools and scale:** The training and evaluation data was created via NS-1 simulations and coupled with an actual vehicular trace data.
**Results:** Accuracy, Precision, Recall, and F1-Score were some of the metrics that were used to evaluate the model with the ten anomaly classes and at different node densities. The semi-supervised labeling and correlation-based filtering of features were also added to the model to improve its generalization.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses misbehaviour/attack detection (direction 1 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
