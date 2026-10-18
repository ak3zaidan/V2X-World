# Testbed for wireless vehicle communication: a simulation approach based on three-phase traffic theory
B. S. Kerner, S. L. Klenov, A. Brakemeier · 2008 IEEE Intelligent Vehicles Symposium 2008 · https://doi.org/10.1109/ivs.2008.4621143 · relevance: high · basis: full text

**Problem:** Coupling VANET and traffic simulation typically uses separate tools.
**Approach:** Testbed integrating vehicle motion (three-phase traffic theory microscopic model), 802.11e channel access, radio propagation and message reception into one model so network and traffic run simultaneously.
**Tools and scale:** Single integrated simulator; per-vehicle/RSU "matrix of signal powers" each time step; reception threshold -116 dBm (results change about 1% vs -126 dBm).
**Results:** Demonstrates integrated simulation of ad-hoc network and traffic flow on one testbed.
**Limitations and future work (as stated by the authors):**
- Accuracy/time trade-off via reception threshold.
**Overlap with our engine:** strong — strong: a single integrated (not co-simulated) traffic + V2X radio simulator already existed in 2008; our single-process design is not new per se.
**Novelty implications:** We cannot claim the first single-process integrated traffic/network V2X simulator; our distinct claims must rest on PKI/SCMS integration, determinism, joint ground truth and counterfactual reruns.
