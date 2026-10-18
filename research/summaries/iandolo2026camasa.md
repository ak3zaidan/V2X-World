# CAMASA: A CAM-based Dataset from the MASA Living Lab
Salvatore Iandolo, Marco Savarese, Gaetano Orazio Cauchi, Antonio Solida et al. · arXiv 2026 · https://arxiv.org/abs/2606.10641 · relevance: high · basis: full text (abstract, conclusion/future-work sections)

**Problem:** Trajectory prediction is a key enabler of autonomous and cooperative driving systems.
**Approach:** However, most existing benchmarks are either sensor-centric, geographically constrained, or based on synthetic mobility traces that do not capture real-world V2X communication dynamics. This paper introduces CAMASA, a large-scale infrastructure-based dataset derived from Cooperative Awareness Messages (CAMs) and Decentralized Environmental Notification Messages (DENMs) collected within the Modena Automotive Smart Area (MASA).
**Tools and scale:** Not stated in the available text unless given above (topic: adjacent V2X security).
**Results:** The dataset comprises more than 40 million CAMs and 2 million DENMs recorded under authentic urban traffic conditions over multiple months. We present a rigorous preprocessing pipeline that includes filtering, pseudonym reconciliation to account for ETSI privacy-driven stationID changes, and temporal normalization to 10 Hz trajectories, suitable for motion forecasting and time-series analysis.
**Limitations and future work (as stated by the authors):**
- "Future work will extend the dataset to include additional ITS message types and to enrich communication-centric anal- yses, including inter-arrival variability, multi-RSU reception overlap, and reliability indicators, to further characterize real- world C-ITS deployments. REFERENCES [1] J. Cheng, X."
**Overlap with our engine:** none — adjacent background
**Novelty implications:** Does not affect novelty claims of the candidate directions beyond background in adjacent V2X security.
