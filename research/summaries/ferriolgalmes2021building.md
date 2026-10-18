# Building a Digital Twin for Network Optimization Using Graph Neural Networks
Miquel Ferriol-Galmés, José Suárez-Varela, Jordi Paillise, Xiang Shi et al. · Computer Networks 2021 · https://doi.org/10.2139/ssrn.3995236 · relevance: medium · basis: abstract

**Problem:** Network modeling is a critical component for Quality of Service (QoS) optimization. Current networks implement Service Level Agreements (SLA) by careful configuration of both routing and queue scheduling policies.
**Approach:** In this paper, we propose a network model able to understand the complex relationship between (i) the queueing policy (scheduling algorithm and queue sizes), (ii) the network topology, (iii) the routing configuration, and (iv) the input traffic matrix. We call our model TwinNet, a Digital Twin that can accurately estimate relevant SLA metrics for network optimization.
**Tools and scale:** Mentioned in abstract: graph neural, digital twin, real-world. (From the abstract only.)
**Results:** Our results show that TwinNet can provide accurate estimates of end-to-end path delays in 106 unseen real-world topologies, under different queuing configurations (MRE=3.8%).
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** partial — GNN network digital twin (RouteNet-style) as simulator surrogate.
**Novelty implications:** Learned network surrogates exist (wired); V2X surrogates open.
