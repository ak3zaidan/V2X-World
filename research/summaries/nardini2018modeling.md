# Modeling Network-Controlled Device-to-Device Communications in SimuLTE
Giovanni Nardini, Antonio Virdis, Giovanni Stea · Sensors 2018 · https://doi.org/10.3390/s18103551 · relevance: high · basis: full text

**Problem:** Simulating network-controlled LTE-A D2D (sidelink) efficiently.
**Approach:** D2D extension of SimuLTE (OMNeT++/INET): logical/physical transmission split, frequency-reuse scheduling, mode switching, broadcast via multicast groups and transmission horizons.
**Tools and scale:** OMNeT++, SimuLTE, INET; LTE-A D2D; multihop broadcast scenarios.
**Results:** D2D simulation is not significantly costlier than cellular simulation of comparable scale; heaviest cost is sidelink channel quality estimation.
**Limitations and future work (as stated by the authors):**
- Mode switching risks data loss due to PDCP/RLC interplay (noted)
- further extensions discussed as future work.
**Overlap with our engine:** partial — LTE sidelink simulation modelling, related to our LTE-V2X stack.
**Novelty implications:** Efficient sidelink modelling is prior art.
