# Interference Detection and Reporting in IEEE 802.11p Connected Vehicle Networks
David G. Michelson, Hamed Noori, Quinn Ramsay · IEEE Vehicular Technology Conference 2019 · https://doi.org/10.1109/vtcfall.2019.8891233 · relevance: high · basis: abstract

**Problem:** DSRC networks cannot detect or report interference (e.g. Wi-Fi sharing 5.9 GHz).
**Approach:** Detect interference with existing 802.11p capabilities; report it by cloning the SCMS misbehaviour reporting subsystem to a "Spectrum Misbehavior Authority".
**Tools and scale:** Demonstration with 802.11p.
**Results:** Interference detection feasible simply and inexpensively.
**Limitations and future work (as stated by the authors):**
- Applies similarly to C-V2X (not tested).
**Overlap with our engine:** partial — reuses SCMS misbehaviour reporting for PHY events; cross-layer reporting idea.
**Novelty implications:** SCMS-reporting reuse for spectrum events already proposed.
