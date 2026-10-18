# Sybil Attacks Detection and Traceability Mechanism Based on Beacon Packets in Connected Automobile Vehicles
Yaling Zhu, Jia Zeng, Fangchen Weng, Dan Han, Yiyu Yang, Xiaoqi Li et al. · Sensors 2024 · https://doi.org/10.3390/s24072153 · relevance: high · basis: full text

**Problem:** Sybil defences detect fake identities but do not trace the malicious source.
**Approach:** RSUs instruct vehicles to broadcast/listen customised key beacons, build a neighbour graph, compute edge success probability for credibility, detecting Sybils and tracing attackers.
**Tools and scale:** Simulation (details not extracted).
**Results:** Precision 98.53% and recall 95.93%; real-time detection and tracking.
**Limitations and future work (as stated by the authors):**
- Discusses limitations of other works (location-bound detectors, cost, class imbalance).
**Overlap with our engine:** partial — Sybil source tracing; in SCMS this is done by MA/LA linkage, which they do not model.
**Novelty implications:** Physical-presence beacons for tracing exist.
