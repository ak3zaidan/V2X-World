# Federated Reinforcement Learning to Optimize Teleoperated Driving Networks
Filippo Bragato, Marco Giordani, Michele Zorzi · GLOBECOM 2024 - 2024 IEEE Global Communications Conference 2024 · https://doi.org/10.1109/globecom52923.2024.10901349 · relevance: low · basis: abstract

**Problem:** Several sixth generation (6G) use cases have tight requirements in terms of reliability and latency, in particular teleoperated driving (TD).
**Approach:** To address those requirements, Predictive Quality of Service (PQoS), possibly combined with reinforcement learning (RL), has emerged as a valid approach to dynamically adapt the configuration of the TD application (e.g., the level of compression of automotive data) to the experienced network conditions. In this work, we explore different classes of RL algorithms for PQoS, namely MAB (stateless), SARSA (stateful on-policy), Q-Learning (stateful off-policy), and DSARSA and DDQN (with Neural Network (NN) approximation).
**Tools and scale:** Not stated in the abstract.
**Results:** We trained the agents in a federated learning (FL) setup to improve the convergence time and fairness, and to promote privacy and security. We show that Q-Learning uses a small number of learnable parameters, and is the best approach to perform PQoS in the TD scenario in terms of average reward, convergence, and computational cost.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — adjacent background; no V2X security, PKI or simulator component.
**Novelty implications:** Nothing in our candidate directions is ruled out; useful only as background.
