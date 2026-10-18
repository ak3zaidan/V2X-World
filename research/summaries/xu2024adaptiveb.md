# Adaptive Policy Learning for Connected Autonomous Vehicles Defending Malicious Access Requests by Graph Reinforcement Learning
Qian Xu, Lei Zhang, Yixiao Liu · IEEE Internet of Things Journal 2024 · https://doi.org/10.1109/jiot.2024.3429522 · relevance: low · basis: abstract

**Problem:** Access requests are used in cooperative tasks among connected and automated vehicles (CAVs). Authorization decisions are determined by dynamic trust values in trust value-based access control (TBAC).
**Approach:** Policy learning for connected autonomous vehicles by graph reinforcement learning (VPolicy-GRL) is proposed, enhancing the conventional deep Q network (DQN) with a graph neural network (GNN) framework. First, a novel three-layer framework of TBAC for CAVs is proposed.
**Tools and scale:** Vehicular setting (VANET/IoV/V2X). Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** Results demonstrate that VPolicy-GRL achieves superior feature abstraction with STGCN compared to graph convolutional network, which also obtains higher true rejection rates than Double DQN and DQN for both unauthorized and interfering access requests.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial: vehicular security topic, but no simulation of traffic, radio, SCMS backend or safety outcomes is described.
**Novelty implications:** Rules out nothing specific in our candidate directions. It shows that lightweight V2X authentication protocols are widely proposed; we should not claim a new authentication scheme. Open: evaluation of such schemes in a closed-loop city-scale simulation with radio and safety ground truth.
