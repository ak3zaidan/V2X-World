# Performance Analysis of C-V2X Mode 4 Communication Introducing an Open-Source C-V2X Simulator
Fabian Eckermann, Moritz Kahlert, Christian Wietfeld · 2019 IEEE 90th Vehicular Technology Conference (VTC2019-Fall) 2019 · https://doi.org/10.1109/vtcfall.2019.8891534 · relevance: high · basis: full text

**Problem:** No open-source C-V2X Mode 4 simulator existed.
**Approach:** Open-source ns-3 C-V2X Mode 4 simulator (LTE Rel-14 sidelink, sensing-based SPS).
**Tools and scale:** ns-3; worst case 250 vehicles in 100 m x 100 m; Manhattan grid.
**Results:** Scalable to 250 vehicles within LTE Rel-14 requirements; PIR at most 100 ms for >99% of transmissions; analysed reservation period and reselection probability effects on PRR.
**Limitations and future work (as stated by the authors):**
- None explicit in extracted sections.
**Overlap with our engine:** partial — partial: LTE-V2X Mode 4 stack, which our engine also implements (matched to OpenCV2X rather than this).
**Novelty implications:** Nothing of our candidate directions is ruled out.
