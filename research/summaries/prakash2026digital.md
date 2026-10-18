# A Digital Twin Based Hybrid V2X Communication Framework for Scalable Simulation and Hardware Validation
Vinayak Prakash, Nikhil Denniss Baby, Rohan R Krishnan, Nandhu Babu et al. · 2026 International Conference on Smart Communication and Sustainable Technologies (ICSCST) 2026 · https://doi.org/10.1109/icscst70031.2026.11709825 · relevance: high · basis: abstract

**Problem:** The rapid deployment of Connected and Autonomous Vehicles (CAVs) necessitates highly reliable and low-latency Vehicle-to-Everything (V2X) communication networks. As traffic density increases, static communication routing leads to severe network congestion, degrading the Packet Delivery Ratio (PDR) and increasing critical safety latency.
**Approach:** To address this challenge, we propose a novel hybrid Digital Twin framework that bridges a physical hardware-in-the-loop (HIL) prototype with a scalable traffic simulation. Furthermore, we formulate the dynamic V2X mode selection problem—switching between Vehicle-to-Vehicle (V2V) and Vehicle-to-Infrastructure (V2I) networks—as a Markov Decision Process (MDP).
**Tools and scale:** Mentioned in abstract: SUMO, reinforcement learning, digital twin, real-world. (From the abstract only.)
**Results:** Using a Proximal Policy Optimization (PPO) reinforcement learning agent, our system dynamically allocates communication modes to optimize the trade-off between the high reliability of V2I and the ultra-low latency of V2V. Experimental results from the synchronized Digital Twin indicate that the proposed PPO-based adaptive controller reduces average communication latency relative to a static V2I baseline while maintaining near-perfect simulated packet delivery performance under varying traffic densities.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** partial — SUMO-based V2X twin with hardware-in-the-loop and RL mode selection.
**Novelty implications:** No security layer.
