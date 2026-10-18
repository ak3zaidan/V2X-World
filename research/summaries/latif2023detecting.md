# Detecting Contextual Network Anomalies with Graph Neural Networks
Hamid Latif, José Suárez-Varela, Albert Cabellos-Aparicio, Pere Barlet-Ros · Proceedings of the 2nd on Graph Neural Networking Workshop 2023 2023 · https://doi.org/10.1145/3630049.3630171 · relevance: low · basis: abstract

**Problem:** Detecting anomalies on network traffic is a complex task due to the massive amount of traffic flows in today's networks, as well as the highly-dynamic nature of traffic over time.
**Approach:** In this paper, we propose the use of Graph Neural Networks (GNN) for network traffic anomaly detection. We formulate the problem as contextual anomaly detection on network traffic measurements, and propose a custom GNN-based solution that detects traffic anomalies on origin-destination flows.
**Tools and scale:** Not stated in the abstract.
**Results:** The results show that the anomalies detected by our solution are quite complementary to those captured by the baselines (with a max. of 36.33% overlapping anomalies for PCA). Moreover, we manually inspect the anomalies detected by our method, and find that a large portion of them can be visually validated by a network expert (64% with high confidence, 18% with mid confidence, 18% normal traffic).
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — touches learned network models but not the joint traffic/radio/SCMS closed loop.
**Novelty implications:** Nothing in our candidate directions is ruled out; useful only as background on learned network models.
