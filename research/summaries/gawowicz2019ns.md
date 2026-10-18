# ns-3 meets OpenAI Gym
Piotr Gawłowicz, Anatolij Zubow · Proceedings of the 22nd International ACM Conference on Modeling, Analysis and Simulation of Wireless and Mobile Systems 2019 · https://doi.org/10.1145/3345768.3355908 · relevance: medium · basis: abstract

**Problem:** Recently, we have seen a boom of attempts to improve the operation of networking protocols using machine learning techniques. The proposed reinforcement learning (RL) based control solutions very often overtake traditionally designed ones in terms of performance and efficiency.
**Approach:** This paper presents the ns3-gym - the first framework for RL research in networking. Here, we present an illustrative example from the cognitive radio area, where a wireless node learns the channel access pattern of a periodic interferer in order to avoid collisions with it.
**Tools and scale:** Mentioned in abstract: ns-3, NS-3, reinforcement learning. (From the abstract only.)
**Results:** It is based on OpenAI Gym, a toolkit for RL research and ns-3 network simulator. Specifically, it allows representing an ns-3 simulation as an environment in Gym framework and exposing state and control knobs of entities from the simulation for the agent's learning purposes.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** partial — ns3-gym RL interface.
**Novelty implications:** Simulator-ML interfaces exist.
