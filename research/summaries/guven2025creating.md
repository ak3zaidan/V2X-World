# Creating a realistic sybil attack dataset for inter-vehicle communication
Taner Guven, Z. Cihan Taysi · Peer-to-Peer Networking and Applications 2025 · https://doi.org/10.1007/s12083-025-02058-w · relevance: high · basis: full text

**Problem:** No realistic RSSI dataset for Sybil detection under congestion and packet loss.
**Approach:** Veins 5.2/SUMO 1.8 with F2MD application components on 11.2 km² of Istanbul (OSM, more than 1,000 manual fixes), traffic calibrated to municipal data, 10 Hz beacons, log-normal shadowing plus simple obstacle shadowing. Four Sybil data-generation methods × five transmit-power-control modes = 20 attacks. Ground-truth file with attack type, power-control type and Tx power per message.
**Tools and scale:** 25-min sample: 820 GB, 7,876 genuine vehicles, 119 attackers, 652 ghosts, 849 M received genuine messages; 20 days of compute (55–60 days for the full scenario).
**Results:** Traffic NRMSE 0.191 against municipal speeds; RSSI compared with F2MD only qualitatively (Fig. 6); collisions present where LuST has none.
**Limitations and future work (as stated by the authors):**
- Very high compute cost (Veins has no parallelism); primitive power-control attacks; validation against real RSSI measurements still needed (§6).
- Pseudonym change policies deliberately disabled (§3).
**Overlap with our engine:** strong — a PHY-level labelled attack dataset; no PKI, impact labels or rotation.
**Novelty implications:** The default prevalence is stated as 0.15 (§4.4) but measured at 1.5% (§5.3). **The data is withdrawn:** the GitHub README now reads "Do not use or cite this work … Data is restricted for all use cases" (authorship dispute; confirmed by muzammil2026code §IX). It cannot serve as an available comparator. See synthesis/check-datasets-2026.md §1.4.
