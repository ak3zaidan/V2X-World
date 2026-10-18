# A Federated LSTM Autoencoder Framework for Privacy-Preserving Intrusion Detection in V2X Networks
Β. Vishwanath, Chandrasekhar Reddy P · Engineering Technology & Applied Science Research 2026 · https://doi.org/10.48084/etasr.13121 · relevance: medium · basis: abstract

**Problem:** The fast growth of Vehicle-to-Everything (V2X) networks requires privacy-preserving Intrusion Detection Systems (IDSs) for effective operation.
**Approach:** The proposed Federated Long Short-Term Memory Autoencoder (Fed-LSTM-AE) framework allows distributed vehicular clients to perform collaborative anomaly detection through model parameter sharing without exchanging raw data. The framework enables each client to create its own LSTM-based autoencoder model of normal traffic patterns while sharing only model parameters with a central server through federated learning to maintain data privacy and improve system scalability.
**Tools and scale:** Mentioned in abstract: VEREMI, VeReMi
**Results:** The framework enables each client to create its own LSTM-based autoencoder model of normal traffic patterns while sharing only model parameters with a central server through federated learning to maintain data privacy and improve system scalability. Experiments using the VeReMi dataset show that Fed-LSTM-AE achieves better performance than the centralized LSTM, one-dimensional Convolutional Neural Network (1D CNN), Random Forest, and Isolation Forest baseline methods in terms of detection accuracy, F1-score, and Area Under the Receiver Operating Characteristic Curve (ROC-AUC) metrics.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — same area (V2X/vehicular security or networking) with a narrower, mechanism-level focus.
**Novelty implications:** Does not remove any of our claims beyond its specific mechanism; we cannot claim to be first on that mechanism. The joint closed-loop traffic/radio/PKI evaluation with counterfactual ground truth remains open as far as this abstract shows.
