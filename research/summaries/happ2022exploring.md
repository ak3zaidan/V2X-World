# Exploring the Limitations of Current Graph Neural Networks for Network Modeling
Martin Happ, Jia-Lei Du, M. Herlich, Christian Maier, Peter Dorfinger, J. Suárez-Varela · IEEE/IFIP Network Operations and Management Symposium 2022 · https://doi.org/10.1109/noms54207.2022.9789708 · relevance: high · basis: abstract

**Problem:** Generalisation limits of RouteNet.
**Approach:** Systematic evaluation under changes of topology size, capacities, packet size distribution and congestion.
**Tools and scale:** Simulated datasets.
**Results:** Robust to graph structure changes but degrades considerably when target distributions differ from training.
**Limitations and future work (as stated by the authors):**
- New methods needed to cope with traffic loads and network sizes significantly different from training.
**Overlap with our engine:** partial - informs surrogate generalisation in direction 3.
**Novelty implications:** Out-of-distribution generalisation is a known open problem for surrogates.
