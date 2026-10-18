# Digital Twin-Enabled Dynamic Aggregation for Efficient Federated Learning
Wenqin Zhuang, Yuao Wang, Guocheng Wang · Italian National Conference on Sensors 2026 · https://doi.org/10.3390/s26144460 · relevance: low · basis: abstract

**Problem:** Federated learning (FL) enables collaborative model training without sharing raw data, but it faces challenges due to client heterogeneity, leading to inefficiency and reduced accuracy.
**Approach:** This paper proposes a digital twin (DT)-based dynamic FL aggregation method to address these issues. The framework integrates a DT layer on the server side to perform preaggregation evaluations, simulating various aggregation strategies to select the optimal approach before actual global aggregation.
**Tools and scale:** Mentions: federated learning.
**Results:** An adaptive clustering method based on K-means is employed to group clients with similar characteristics, and a hierarchical aggregation evaluation strategy is designed to optimize both intra-cluster and inter-cluster aggregation, with the goal of minimizing latency and energy consumption while maximizing model accuracy. Simulation results on the MNIST and CIFAR-10 datasets demonstrate that the proposed method not only accelerates model convergence and improves accuracy but also significantly reduces training latency and energy consumption costs compared with baseline FL algorithms.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
