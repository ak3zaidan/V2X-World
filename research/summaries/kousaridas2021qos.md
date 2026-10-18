# QoS Prediction for 5G Connected and Automated Driving
Apostolos Kousaridas, Ramya Panthangi Manjunath, Jose Mauricio Perdomo, Chan Zhou, Ernst Zielinski, Steffen Schmitz et al. · IEEE Communications Magazine 2021 · https://doi.org/10.1109/mcom.110.2100042 · relevance: high · basis: full text (extracted sections)

**Problem:** Sudden QoS changes in 5G can affect automated driving safety.
**Approach:** Describes how a 5G system can generate and deliver predictive QoS to V2X applications; tele-operated driving case study.
**Tools and scale:** ns-3.33 with SUMO; 3GPP evaluation methodology; ML prediction of UL throughput.
**Results:** UL throughput prediction MAPE 0.005 with location plus serving/neighbour cell load features.
**Limitations and future work (as stated by the authors):**
- Additional features (channel variation) beyond scope.
- Open research topics identified, e.g., acceptable prediction horizon and accuracy.
**Overlap with our engine:** partial - ns-3+SUMO V2X co-simulation with learned prediction; no security.
**Novelty implications:** Predictive QoS is covered; not related to PKI or MBD.
