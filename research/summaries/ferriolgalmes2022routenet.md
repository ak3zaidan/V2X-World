# RouteNet-Fermi: Network Modeling with Graph Neural Networks
Miquel Ferriol-Galmés, Jordi Paillisse, José Suárez-Varela, Krzysztof Rusek, Shihan Xiao, Xiang Shi et al. · IEEE/ACM Transactions on Networking 2022 · https://doi.org/10.1109/tnet.2023.3269983 · relevance: high · basis: full text (extracted sections)

**Problem:** Queuing theory assumes Markovian traffic; packet simulators are computationally expensive.
**Approach:** RouteNet-Fermi GNN predicting delay, jitter and loss with arbitrary traffic models, routing and queue scheduling.
**Tools and scale:** Networks up to 300 nodes; mixed non-Markovian traffic; physical testbed and real-network packet traces; benchmarked against MimicNet and queuing theory.
**Results:** Delay MRE 6.24% on 1,000 samples with topologies 50-300 nodes; 5.64% on real-world traces; 11% on a physical testbed.
**Limitations and future work (as stated by the authors):**
- No strong mathematical performance guarantees (errors bounded by no-queue and full-queue delays).
- Training data from production networks is infeasible; requires testbeds or simulators.
**Overlap with our engine:** partial - state of the art learned surrogate for packet-level simulation (direction 3); wired networks.
**Novelty implications:** Cannot claim simulator-matching GNN surrogates are novel in general; V2X with mobility, channel, and PKI remains open.
