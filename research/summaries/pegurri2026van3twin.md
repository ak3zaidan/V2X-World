# VaN3Twin: The Multi-Technology V2X Digital Twin With Ray Tracing in the Loop
Roberto Pegurri, Diego Gasco, Francesco Linsalata, Marco Rapelli, Eugenio Moro, Francesco Raviglione et al. · IEEE Transactions on Wireless Communications 2026 · https://doi.org/10.1109/twc.2026.3685710 · relevance: high · basis: full text (extracted sections)

**Problem:** Conventional V2X simulators model LoS/NLoS crudely and cannot study DSRC/C-V2X coexistence accurately.
**Approach:** VaN3Twin: open-source full-stack V2X network DT extending ms-van3t with Sionna ray tracing in the loop, vehicle-mesh blockage, Doppler, and RB-level cross-technology interference tracking.
**Tools and scale:** ns-3 ms-van3t; Sionna RT; DSRC and C-V2X; rural and urban field measurement campaigns in Italy.
**Results:** Application-layer disagreement vs field data reduced 50% (rural) and over 70% (urban) compared with state-of-the-art simulators.
**Limitations and future work (as stated by the authors):**
- Conservative cache invalidation on any mesh update is costly.
**Overlap with our engine:** partial - multi-technology full-stack V2X simulator validated against measurements; strong on PHY; no SCMS/PKI.
**Novelty implications:** We cannot claim the first multi-RAT full-stack V2X simulator or first measurement-validated one. Open: PKI and misbehaviour in such a stack.
