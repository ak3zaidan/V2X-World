# A Distributed Hybrid Intrusion Detection Framework for VANETs using Semantic Embedding, Deep Feature Learning, and Adaptive Ensemble Classification
Bhaskar.SV, Vishnupriya.R · 2026 7th International Conference on Inventive Research in Computing Applications (ICIRCA) 2026 · https://doi.org/10.1109/icirca69024.2026.11570537 · relevance: medium · basis: abstract

**Problem:** Vehicular Ad-hoc Networks (VANETs) are vulnerable to cyberattacks, which may modify data, leading to severe accidents.
**Approach:** To overcome this challenge, a scalable network intrusion detection system that combines Apache Spark distributed processing, Spark RDD-based Word2Vec feature embedding, and Convolutional Neural Network (CNN) deep feature extraction with a stacking ensemble classifier is proposed. The proposed architecture effectively detects and categorizes various attack vectors in VANETs
**Tools and scale:** To effectively address the severe class imbalance in CIC_IDS2017 benchmark dataset, SMOTE-based oversampling is adopted.
**Results:** Transfer learning is utilized to improve the detection capability of emerging new attack patterns. However, a tuned Voting Classifier, integrating Decision Tree, Random Forest and XG Boost, achieves an accuracy of 99.01 percent than other models in other performance tests.
**Limitations and future work (as stated by the authors):**
- "To overcome this challenge, a scalable network intrusion detection system that combines Apache Spark distributed processing, Spark RDD-based Word2Vec feature embedding, and Convolutional Neural Network (CNN) deep feature extraction with a stacking ensemble classifier is proposed."
**Overlap with our engine:** partial — addresses misbehaviour/attack detection (direction 1 area); datasets/twins/surrogates (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
