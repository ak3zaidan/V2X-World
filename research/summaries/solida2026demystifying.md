# Demystifying VEINS: A Reality Check Against Living Lab Experiments
Antonio Solida, Giovanni Gambigliani Zoccoli, Gaetano Orazio Cauchi, Filip Valgimigli et al. · arXiv 2026 · https://arxiv.org/abs/2605.29988 · relevance: high · basis: full text

**Problem:** Safety applications in vehicle-to-everything communications and Cooperative Intelligent Transport Systems rely on reliable and timely message exchange, which in turn depends on accurate modeling of wireless signal propagation. Simulation frameworks such as VEINS are widely adopted to design and evaluate such systems before deployment; however, their realism strongly depends on the validity of the underlying channel and antenna models.
**Approach:** This work presents an empirical validation of the VEINS simulator against real-world data collected from the MASA living laboratory. Using the default configuration, we compare Received Signal Strength Indicator (RSSI), number of messages, and attenuation of the signal.
**Tools and scale:** Veins default configuration vs. MASA living-lab (Modena) IEEE 802.11p measurements; RSSI, message count, attenuation.
**Results:** The results show that VEINS systematically overestimates the RSSI value, while losing approximately 18% of the total number of messages received compared to the MASA, revealing inconsistencies between simulation and reality.
**Limitations and future work (as stated by the authors):**
- Uses only Veins default configuration.
- Future work: "refining the propagation model to better reflect the measurement environment".
- Future work: "tuning VEINS parameters using real data as ground truth".
**Overlap with our engine:** partial — Validates a V2X radio simulator (Veins) against field data; our PHY/MAC is matched to Veins, so its reported bias (RSSI overestimate, ~18% fewer messages) is inherited unless we calibrate.
**Novelty implications:** Shows Veins-matching is not the same as real-world fidelity; calibration of Veins-class models against living-lab data is open and directly relevant to our validation claims.
