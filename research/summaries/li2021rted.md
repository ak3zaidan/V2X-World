# RTED-SD: A Real-Time Edge Detection Scheme for Sybil DDoS in the Internet of Vehicles
Jiabin Li, Zhi Xue, Changlian Li, Ming Liu · IEEE Access 2021 · https://doi.org/10.1109/access.2021.3049830 · relevance: high · basis: full text

**Problem:** Real-time edge detection of Sybil DDoS in IoV.
**Approach:** Entropy of traffic distribution with Fast Quartile Deviation Check, optimised sliding window and incremental entropy; new metric Temporal False Omission Rate.
**Tools and scale:** F2MD (VEINS) DosRandomSybil datasets, with added benign prefixes; rush and low-traffic hours.
**Results:** Detects all Sybil DDoS attacks in F2MD data; average alarm delay 4.92 s; average TFOR 1.60%.
**Limitations and future work (as stated by the authors):**
- None explicitly extracted.
**Overlap with our engine:** partial — measures detection delay (time-aware metric), related to "time to neutralise".
**Novelty implications:** Detection-delay metrics exist; end-to-end to revocation and safety is open.
