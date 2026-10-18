# Development of an In-Vehicle Intrusion Detection Model Integrating Federated Learning and LSTM Networks
Miriam Zambudio Martínez, Rafael Marín-Pérez, A. Gómez-Skarmeta · Inf. 2025 · https://doi.org/10.3390/info16040292 · relevance: medium · basis: abstract

**Problem:** Introduction: Ensuring vehicular cybersecurity is a critical challenge due to the increasing connectivity of modern vehicles, and traditional centralised learning approaches for intrusion detection pose significant privacy risks, as they require sensitive data to be shared from multiple vehicles to a central server. Objective: The aim of this study is therefore to develop an in-vehicle intrusion detection system (IVIDS) that integrates federated 
**Approach:** The proposed system extends previous research by detecting a broader range of attacks (eight types) and exploring different deep learning architectures. Methods: This study employs an extended version of the publicly available VeReMi dataset to train and evaluate multiple neural network architectures, including Multilayer Perceptrons (MLPs), Gated Recurrent Units (GRUs), and Long Short-Term Memory (LSTM) networks.
**Tools and scale:** VeReMi, federated learning, deep learning (mentioned in abstract)
**Results:** The best LSTM model, trained with two previous message lags and standard normalisation, achieved a classification accuracy of 96.75% in detecting eight types of attacks, surpassing previous studies, and demonstrating the potential of applying neural networks designed to work with time series data.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — In-vehicle/CAN or keyless-entry security; outside our V2X network/PKI scope.
**Novelty implications:** No effect on our novelty claims; background only.
