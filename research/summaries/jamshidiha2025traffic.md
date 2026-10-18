# A Traffic-Aware Graph Neural Network for User Association in Cellular Networks
Saeed Jamshidiha, Vahid Pourahmadi, Abbas Mohammadi · IEEE Transactions on Mobile Computing 2025 · https://doi.org/10.1109/tmc.2025.3545464 · relevance: low · basis: abstract

**Problem:** In this paper, we utilize a graph neural network to perform joint user association and access point activation/deactivation to optimize network blockage.
**Approach:** Instead of using theoretical models to characterize the distribution of network traffic, we use a real-world network traffic dataset. In order to train the graph neural network, our method leverages reinforcement learning, specifically employing the deep deterministic policy gradient (DDPG) algorithm.
**Tools and scale:** Mentioned in abstract: dataset.
**Results:** The graph neural network's awareness of the network topology gives the proposed method a clear performance advantage over the commonly used greedy heuristic optimization method, reducing blockage by up to more than 50% on real-world network traffic data while also reducing computational complexity from $\mathcal {O}(N^{3})$O(N3) to $\mathcal {O}(N)$O(N).
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — touches learned network models but not the joint traffic/radio/SCMS closed loop.
**Novelty implications:** Nothing in our candidate directions is ruled out; useful only as background on learned network models.
