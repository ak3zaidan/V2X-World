# Traffic Co-Simulation Framework Empowered by Infrastructure Camera Sensing and Reinforcement Learning
Talha Azfar, Kaicong Huang, Andrew Tracy, Sandra Misiewicz, Chenxi Liu, Ruimin Ke · Journal of Intelligent Transportation Systems, 2025 2024 · https://doi.org/10.1080/15472450.2025.2559410 · relevance: medium · basis: abstract

**Problem:** Traffic simulations are commonly used to optimize urban traffic flow, with reinforcement learning (RL) showing promising potential for automated traffic signal control, particularly in intelligent transportation systems involving connected automated vehicles.
**Approach:** Multi-agent reinforcement learning (MARL) is particularly effective for learning control strategies for traffic lights in a network using iterative simulations. However, existing methods often assume perfect vehicle detection, which overlooks real-world limitations related to infrastructure availability and sensor reliability.
**Tools and scale:** Mentions: CARLA, real-world, SUMO; scale not stated in the abstract.
**Results:** MARL agents trained with four different reward structures leverage this visual feedback to optimize signal timings and improve network-wide traffic flow. Results show that while better accuracy improves performance, MARL agents can still achieve significant improvements with imperfect detection, demonstrating scalability and adaptability for real-world scenarios.
**Limitations and future work (as stated by the authors):**
- However, existing methods often assume perfect vehicle detection, which overlooks real-world limitations related to infrastructure availability and sensor reliability.
**Overlap with our engine:** partial — addresses V2X simulation/datasets (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
