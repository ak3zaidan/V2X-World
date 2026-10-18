# Low-Latency Relay Selection in NR-V2X Vehicular Communications via Graph Isomorphism Networks with Edge Features
Giambattista Amati, Federica Mangiatordi, Emiliano Pallotti, Simone Angelini et al. · arXiv 2026 · https://doi.org/10.1109/iccspa69228.2026.11600864 · relevance: medium · basis: abstract

**Problem:** Reliable, low-latency uplink connectivity is a key requirement for C-V2X networks in dense urban environments, where fast channel variations and blockages often degrade direct vehicle-to-infrastructure links. Multi-hop relaying can restore coverage, but relay-link activation under radio, capacity, and routing constraints results in an NP-hard optimisation ...
**Approach:** This paper introduces an edge-aware Learning-to-Optimise framework for real-time relay selection.
**Tools and scale:** Experiments on a large-scale dataset generated via an OSM-SUMO-GEMV$^2$ pipeline show that GINE closely matches MILP decisions at the link level (accuracy 0.9589), F1-score (0.9544) on validation) and yields consistent end-to-end connectivity gains over a 1-hop MILP baseline (up to 9.2% with four ...
**Results:** Inference latency remains tightly bounded, with all evaluated instances completing within 5~ms. Moreover, GP-MILP preserves MILP-equivalent solutions (same objective value) while achieving solver runtimes below 30~ms for more than 98%) of the graph instances, making MILP-grade optimisation compatible with stringent NR-V2X latency budgets.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — Bespoke VANET/IoV authentication or privacy scheme (CPPA/certificateless/batch/blockchain style), evaluated by analysis or small simulation; not IEEE 1609.2/SCMS and no closed-loop safety evaluation.
**Novelty implications:** Adds to the large body of standalone authentication schemes; does not touch end-to-end SCMS simulation or safety impact of security.
