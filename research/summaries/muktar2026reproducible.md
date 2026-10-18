# A Reproducible UAV-Assisted VANET Dataset Generator for Fragmentation Risk Analysis in Intelligent Transportation Systems
Bappa Muktar, Justin Moskolaï Ngossaha, Adama Nouboukpo · arXiv 2026 · https://arxiv.org/abs/2606.01488 · relevance: high · basis: full text

**Problem:** Vehicular Ad Hoc Networks (VANETs) are a key component of Intelligent Transportation Systems, enabling cooperative communication among vehicles and between vehicles and roadside infrastructure. However, their highly dynamic topology makes them vulnerable to network fragmentation, particularly in highway scenarios, low-density traffic conditions, localized accident zones, and communication-stressed environments.
**Approach:** This paper proposes a reproducible UAV-assisted VANET dataset generator for short-term fragmentation risk prediction. By providing a modular, extensible, and reproducible ns-3-based data-generation framework, this work offers a practical basis for future supervised learning studies and connectivity management strategies in UAV-assisted VANETs.
**Tools and scale:** ns-3, two-lane opposite-direction highway, UAVs as aerial relays; profiles: free-flow, accident, sparse, dense bursty, mixed stress; future-labelled fragmentation samples.
**Results:** No quantitative results stated in the abstract.
**Limitations and future work (as stated by the authors):**
- Future work: "realistic SUMO-based mobility".
- "richer wireless channel models".
- "larger multi-seed datasets".
- "learning-based UAV control policies".
**Overlap with our engine:** partial — Reproducible, seeded simulation-based dataset generator with future (horizon) labels; no PKI/security, simple mobility.
**Novelty implications:** Seeded labelled-dataset generation from a network simulator is not new; per-unit counterfactual labels and security ground truth remain open.
