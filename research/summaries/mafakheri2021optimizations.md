# Optimizations for Hardware-in-the-Loop-Based V2X Validation Platforms
Babak Mafakheri, Pierpaolo Gonnella, Alessandro Bazzi, Barbara Mavi Masini, Michele Caggiano, Roberto Verdone · 2021 IEEE 93rd Vehicular Technology Conference (VTC2021-Spring) 2021 · https://doi.org/10.1109/vtc2021-spring51267.2021.9448667 · relevance: high · basis: full text

**Problem:** Real-time HIL V2X validation needs fast simulation of positioning and channel impairments.
**Approach:** Models correlated GNSS error and channel propagation with optimisations trading minor accuracy for speed; sidelink LTE/5G-V2X or 802.11p/bd.
**Tools and scale:** HIL platform; measured GNSS error (DRMS 2.32 m, correlation time 10 s).
**Results:** More than one order of magnitude speed increase with minor accuracy loss.
**Limitations and future work (as stated by the authors):**
- MAC protocols assumed ideal without collision losses
- models for density, MCS and message size to be derived.
**Overlap with our engine:** partial — GNSS error modelling and real-time V2X abstraction.
**Novelty implications:** No impact.
