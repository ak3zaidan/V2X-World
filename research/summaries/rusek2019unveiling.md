# Unveiling the potential of Graph Neural Networks for network modeling and optimization in SDN
Krzysztof Rusek, José Suárez-Varela, Albert Mestres, Pere Barlet-Ros, Albert Cabellos-Aparicio · Proceedings of the 2019 ACM Symposium on SDN Research 2019 · https://doi.org/10.1145/3314148.3314357 · relevance: high · basis: full text (extracted sections)

**Problem:** Existing network modelling cannot estimate delay and jitter accurately enough for SDN routing optimisation.
**Approach:** First RouteNet GNN (message passing between paths and links) predicting per source-destination mean delay and jitter; use cases in routing optimisation.
**Tools and scale:** Own packet-level simulator; 260,000 training samples from NSF topology with ~100 routing schemes; 30,000 evaluation samples; code and data public.
**Results:** Worst-case R^2 = 0.86 on unseen topologies, routing and traffic.
**Limitations and future work (as stated by the authors):**
- Trained on a single topology; generalisation shown to other topologies but limited in scope.
**Overlap with our engine:** partial - learned surrogate for packet-level network simulation (direction 3), wired only.
**Novelty implications:** Same as RouteNet journal version: surrogate idea is established; V2X/mobility/security surrogates remain open.
