# Using Dual Attention BiLSTM to Predict Vehicle Lane Changing Maneuvers on Highway Dataset
Farzeen Ashfaq, Rania M. Ghoniem, N. Z. Jhanjhi, Navid Ali Khan, Abeer D. Algarni · Systems 2023 · https://doi.org/10.3390/systems11040196 · relevance: medium · basis: abstract

**Problem:** In this research, we address the problem of accurately predicting lane-change maneuvers on highways.
**Approach:** Lane-change maneuvers are a critical aspect of highway safety and traffic flow, and the accurate prediction of these maneuvers can have significant implications for both. However, current methods for lane-change prediction are limited in their ability to handle naturalistic driving scenarios and often require large amounts of labeled data.
**Tools and scale:** Not stated in the abstract.
**Results:** Our results show that the BiLSTM-based approach with the sliding window technique effectively predicts lane changes with 86% test accuracy and a test loss of 0.325 by considering the context of the input data in both the past and future. Furthermore, the model achieved an accuracy of 83.65% with a loss value of 0.3306 on the other half of the data samples, and the validation accuracy was observed to improve over these epochs, reaching the highest validation accuracy of 92.53%.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses V2X simulation/datasets (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
