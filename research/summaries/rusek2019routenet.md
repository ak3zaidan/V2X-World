# RouteNet: Leveraging Graph Neural Networks for network modeling and optimization in SDN
Krzysztof Rusek, José Suárez-Varela, Paul Almasan, Pere Barlet-Ros, Albert Cabellos-Aparicio · IEEE Journal on Selected Areas in Communications 2019 · https://doi.org/10.1109/jsac.2020.3000405 · relevance: high · basis: full text (extracted sections)

**Problem:** Analytical network models (queuing theory) cannot accurately predict per-path delay, jitter and loss in SDN at low cost.
**Approach:** RouteNet, a GNN with a path-link message-passing scheme that learns the relation between topology, routing and traffic matrix and outputs per source-destination delay distribution and loss; used for routing optimisation and planning.
**Tools and scale:** Wired packet-level simulator datasets on NSF, GBN, GEANT2 and Germany50 topologies (14-50 nodes); 112,000 test samples; TensorFlow. No vehicular or wireless component.
**Results:** Worst-case MRE 15.4% on unseen topologies/routing/traffic; test-set MRE delay 0.022 vs 0.124 for a queuing-theory baseline; inference for a 200-node network about 99 ms on GPU.
**Limitations and future work (as stated by the authors):**
- Accuracy degrades as inputs move away from the training distribution; wider training distributions are needed to generalise.
- Architecture estimates path-level metrics; link-level metrics require modification.
**Overlap with our engine:** partial - learned surrogate of a packet-level network simulator, the generic idea behind candidate direction 3, but wired SDN, no mobility, radio or security.
**Novelty implications:** We cannot claim GNN surrogates of network simulators are new. Open: surrogates for wireless/V2X with mobility, PKI and counterfactual ground truth.
