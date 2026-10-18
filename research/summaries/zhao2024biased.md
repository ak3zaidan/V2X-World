# Biased Backpressure Routing Using Link Features and Graph Neural Networks
Zhongyuan Zhao, Bojan Radojičić, Gunjan Verma, Ananthram Swami, Santiago Segarra · IEEE Transactions on Machine Learning in Communications and Networking 2024 · https://doi.org/10.1109/tmlcn.2024.3461711 · relevance: low · basis: abstract

**Problem:** To reduce the latency of Backpressure (BP) routing in wireless multi-hop networks, we propose to enhance the existing shortest path-biased BP (SP-BP) and sojourn time-based backlog metrics, since they introduce no additional time step-wise signaling overhead to the basic BP.
**Approach:** Rather than relying on hop-distance, we introduce a new edge-weighted shortest path bias built on the scheduling duty cycle of wireless links, which can be predicted by a graph convolutional neural network based on the topology and traffic of wireless networks. Additionally, we tackle three long-standing challenges associated with SP-BP: optimal bias scaling, efficient bias maintenance, and integration of delay awareness.
**Tools and scale:** Not stated in the abstract.
**Results:** Numerical experiments show that our solutions can effectively address the major drawbacks of slow startup, random walk, and the last packet problem in basic BP, improving the end-to-end delay of existing low-overhead BP algorithms under various settings of network traffic, interference, and mobility.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — touches learned network models but not the joint traffic/radio/SCMS closed loop.
**Novelty implications:** Nothing in our candidate directions is ruled out; useful only as background on learned network models.
