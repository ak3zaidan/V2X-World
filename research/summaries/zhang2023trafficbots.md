# TrafficBots: Towards World Models for Autonomous Driving Simulation and Motion Prediction
Zhejun Zhang, Alexander Liniger, Dengxin Dai, Fisher Yu, Luc Van Gool · arXiv 2023 · https://doi.org/10.1109/icra48891.2023.10161243 · relevance: low · basis: abstract

**Problem:** Data-driven simulation has become a favorable way to train and test autonomous driving algorithms.
**Approach:** The idea of replacing the actual environment with a learned simulator has also been explored in model-based reinforcement learning in the context of world models. In this work, we show data-driven traffic simulation can be formulated as a world model.
**Tools and scale:** Mentioned in abstract: dataset, simulate, simulation, simulator, simulators.
**Results:** To improve the scalability, we present a new scheme of positional encoding for angles, allowing all agents to share the same vectorized context and the use of an architecture based on dot-product attention. Experiments on the Waymo open motion dataset show TrafficBots can simulate realistic multi-agent behaviors and achieve good performance on the motion prediction task.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — touches vehicular networking, world models, network/traffic simulation but not the joint traffic/radio/SCMS closed loop.
**Novelty implications:** Nothing in our candidate directions is ruled out; useful only as background on vehicular networking.
