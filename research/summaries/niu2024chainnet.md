# ChainNet: A Customized Graph Neural Network Model for Loss-Aware Edge AI Service Deployment
Zifeng Niu, Manuel Roveri, Giuliano Casale · 2024 54th Annual IEEE/IFIP International Conference on Dependable Systems and Networks (DSN) 2024 · https://doi.org/10.1109/dsn58291.2024.00034 · relevance: low · basis: abstract

**Problem:** Edge AI seeks for the deployment of deep neural network (DNN) based services across distributed edge devices, embedding intelligence close to data sources.
**Approach:** Due to capacity constraints at the edge, a difficult challenge lies in planning a dependable deployment that minimizes the data loss rate so as to meet application Quality-of-Service (QoS) goals. In this paper, we present ChainNet, a customized graph neural network (GNN) model serving as a surrogate to assess the reliability of alternative deployments and guide the loss-aware search for an optimal edge AI deployment plan.
**Tools and scale:** Mentioned in abstract: simulation.
**Results:** Extensive results show that ChainNet delivers a substantial improvement in loss prediction accuracy by over 50% compared to established GNN models, such as graph attention networks (GATs). Moreover, we show that ChainNet provides significantly more dependable deployment decisions under a fixed time budget compared to simulation-based search across a spectrum of systems from small to large-scale.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — touches network/traffic simulation, learned surrogates, learned network models but not the joint traffic/radio/SCMS closed loop.
**Novelty implications:** Nothing in our candidate directions is ruled out; useful only as background on network/traffic simulation.
