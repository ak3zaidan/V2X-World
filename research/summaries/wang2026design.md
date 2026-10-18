# Design of a Wireless Resource Dynamic Scheduling Algorithm Based on Network Digital Twins
Xiaodong Wang, Wei Zhu, Jun Liu et al. · 2026 IEEE 2nd International Conference on Electronics, Energy Systems and Power Engineering (EESPE) 2026 · https://doi.org/10.1109/eespe68405.2026.11649313 · relevance: low · basis: abstract

**Problem:** The proliferation of various wireless services within 5G and 6G networks places increasing demands on the radio access network (RAN) to provide a high degree of dynamic quality of services (QoS).
**Approach:** Dynamic wireless resource scheduling is one method to respond to these demands. One approach to implementing dynamic scheduling is to use a network digital twin (NDT) to train a deep reinforcement learning (DRL) algorithm to schedule radio resources within a RAN.
**Tools and scale:** Mentions: 5G, reinforcement learning.
**Results:** By employing an NDT to train the DRL algorithm, the RAN can significantly reduce the time required to train the algorithm and the network's training overhead. Simulation results using parameters from 3GPP TR 36.814 for six algorithms indicate that the NDT-DRL algorithm outperformed all algorithms, achieving a throughput of 330 bps/Hz, an SLA of 91.2%, and a latency of 18.3 ms.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
