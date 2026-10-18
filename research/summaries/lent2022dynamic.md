# Dynamic Routing in Challenged Networks with Graph Neural Networks
Ricardo Lent · 2022 IEEE Latin-American Conference on Communications (LATINCOM) 2022 · https://doi.org/10.1109/latincom56090.2022.10000566 · relevance: low · basis: abstract

**Problem:** Challenged networks are characterized by a time-varying operational environment that constrains the optimality of standard routing algorithms.
**Approach:** This work investigates a deep learning method that tackles the bundle routing problem by taking advantage of the available network metrics and performance data through a graph neural network (GNN). A cognitive routing decision unit is formulated by defining a GNN structure that accepts both edge and node input features, and that is trained with reinforcement learning.
**Tools and scale:** Not stated in the abstract.
**Results:** Simulation results demonstrate that the proposed cognitive routing method is able to learn how to optimize the next-hop for each data bundle of a flow to achieve lower end-to-end delivery latency than the standard Contact Graph Routing algorithm. The GNN achieves the optimization by detecting and avoiding the extended wait times caused by both butter congestion and the stall times for the next contact when long link disruptions occur.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — touches network/traffic simulation, learned network models but not the joint traffic/radio/SCMS closed loop.
**Novelty implications:** Nothing in our candidate directions is ruled out; useful only as background on network/traffic simulation.
