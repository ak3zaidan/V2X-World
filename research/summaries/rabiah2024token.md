# Token-based Vehicular Security System (TVSS): Scalable, Secure, Low-latency Public Key Infrastructure for Connected Vehicles
Abdulrahman Bin Rabiah, Anas Alsoliman, Yugarshi Shashwat, Silas Richelson et al. · arXiv (Cornell University) 2024 · https://arxiv.org/abs/2402.18365 · relevance: high · basis: full text (abstract, conclusion/future-work sections)

**Problem:** Connected and Autonomous vehicles stand to drastically improve the safety and efficiency of the transportation system in the near future while also reducing pollution.
**Approach:** These systems leverage communication to coordinate among vehicles and infrastructure in service of a number of safety and efficiency driver assist and even fully autonomous applications. Attackers can compromise these systems in a number of ways including by falsifying communication messages, making it critical to support security mechanisms that can operate and scale in dynamic scenarios.
**Tools and scale:** Not stated in the available text unless given above (topic: adjacent V2X security).
**Results:** Towards this end, we present TVSS, a new VPKI system which improves drastically over prior work in the area (including over SCMS; the US department of transportation standard for VPKI). It also results in several security advantages: (1) Scalable Revocation: it greatly simplifies the revocation problem, a difficult problem in large scale certificate systems; and (2) Faster Refresh: Vehicles interact more frequently with the system to refresh thei...
**Limitations and future work (as stated by the authors):**
- "in the future."
- Connected and Autonomous vehicles stand to drastically improve the safety and efficiency of the transportation system in the near future while also reducing pollution.
**Overlap with our engine:** strong — proposes a full VPKI design with formal unforgeability/anonymity/unlinkability definitions and a clone attack; overlaps our SCMS backend and pseudonym provisioning
**Novelty implications:** Prior work on this exact mechanism; we cannot claim it as new. Open: integration with full radio/traffic stack, per-message ground truth, and safety outcomes in closed loop.
