# RouteNet-Erlang: A Graph Neural Network for Network Performance Evaluation
Miquel Ferriol-Galmés, Krzysztof Rusek, José Suárez-Varela, Shihan Xiao, Xiangle Cheng, Pere Barlet-Ros et al. · IEEE INFOCOM 2022 - IEEE Conference on Computer Communications 2022 · https://doi.org/10.1109/infocom48880.2022.9796944 · relevance: high · basis: full text (extracted sections)

**Problem:** Queuing theory imposes strong assumptions on arrival processes that do not hold in real networks.
**Approach:** RouteNet-Erlang GNN supporting complex traffic models, multi-queue scheduling and routing policies.
**Tools and scale:** Simulated datasets with diverse traffic models; topologies an order of magnitude larger than training.
**Results:** Outperforms a state-of-the-art QT model in all scenarios.
**Limitations and future work (as stated by the authors):**
- Requires representative training datasets covering many operational regimes; generating them from production networks is infeasible.
**Overlap with our engine:** partial - learned network surrogate (direction 3).
**Novelty implications:** As other RouteNet papers.
