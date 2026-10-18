# A Deep-Learning based Cascading Framework for Intrusion Detection and Attack Classification on V2X-based Autonomous Vehicles
Mihir Bapat, Yash Parikh · International conference Computer Networks and Inventive Communication Technologies 2026 · https://doi.org/10.1109/iccnct68477.2026.11590071 · relevance: medium · basis: abstract

**Problem:** Vehicular Ad-Hoc Networks (VANETs) facilitate communication among autonomous vehicles (AVs) via V2X (Vehicle-to-Everything), making these AVs susceptible to various cyberattacks, including denial-of-service and data manipulation.
**Approach:** However, to the best of our knowledge, existing intrusion detection models may still train on messages sent by vehicles present in both training and testing, leading to unrealistic pattern recognition that reduces the framework's effectiveness when deployed. Thus, this work proposes a two-stage approach to address this issue: the first is a binary detection model classifying packet windows (sequences) as malicious or benign, while the second part classifies the attack type of malicious sequences.
**Tools and scale:** Not stated in the abstract.
**Results:** In total, Stage 1 used 24 engineered features derived from position, velocity, and acceleration to develop a lightweight LSTM model providing 96.26% accuracy and an ROC-AUC of 97.6%. This model was further optimized for sequential processing to become viable for real-world scenarios, achieving an accuracy of 84.58%, and an ROC-AUC of 96.2%.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses misbehaviour/attack detection (direction 1 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
