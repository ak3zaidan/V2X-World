# How well can your car be tracked: Analysis of the European C-ITS pseudonym scheme
Stephan Escher, Markus Sontowski, Knut Berling, S. Köpsell et al. · IEEE Vehicular Technology Conference 2021 · https://doi.org/10.1109/vtc2021-spring51267.2021.9449078 · relevance: high · basis: abstract

**Problem:** How trackable are vehicles under the European C-ITS pseudonym change strategy?
**Approach:** Simulate realistic urban traffic (Luxembourg) and attack the recommended pseudonym change scheme.
**Tools and scale:** Luxembourg urban traffic simulation (LuST-type); attacker linking pseudonyms using traffic statistics and static V2X info.
**Results:** Linking with traffic statistics is harder than prior work suggests; adding static info (vehicle length/width) enormously improves linking; privacy depends on vehicle density and properties and attacker coverage.
**Limitations and future work (as stated by the authors):**
- Results suggest C-ITS pseudonym scheme alone does not guarantee location privacy
**Overlap with our engine:** partial — Pseudonym linkability with static message fields; relates to cross-layer linkability runner-up.
**Novelty implications:** Static-field linkability shown; joint radio-fingerprint plus PKI-timing linkability remains open.
