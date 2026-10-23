# Enhancing V2X Security Through Combined Rule-Based and DL-Based Local Misbehavior Detection in Roadside Units
Seung-Young Park, Duksoo Kim, Seokwoo Lee · IEEE Open Journal of Intelligent Transportation Systems 5:656-668, 2024 · https://doi.org/10.1109/ojits.2024.3479716 · relevance: high · basis: full text

**Problem:** RSUs see only truncated BSM sequences from passing vehicles, so deep-learning local MBD degrades. Central forwarding adds delay.
**Approach:**
- RSUs forward BSMs to a MEC server.
- Rule-based pre-screening with **CaTch** ("the publicly available implementations and operational parameters"), run on one BSM or two BSMs "received within three-time intervals".
- The BSMs it passes then go to a GRU or bidirectional-LSTM autoencoder.
- Detection results are fused across RSUs using pseudonym IDs, and optionally real IDs "similar to techniques used within the SCMS" (LA-based linkage).
**Tools and scale:** A 24 h simulation of BSMs from normal and misbehaving vehicles in a VeReMi/F2MD-style setup, with per-RSU reception counts (Table 2). No real data, no US SCMS Manager rules, no MA decision or revocation modelled.
**Results:** The combined method lowers the FNR at a fixed FPR of 10⁻³. Multi-RSU fusion keyed by real ID is the most consistent across faulty and attack types.
**Limitations and future work (as stated by the authors):** Fixed thresholds "may not directly translate to optimal performance in real-world scenarios". Adaptive thresholding is left to future work.
**Overlap with our engine:** partial. RSU-side detection and LA-linked aggregation.
**Novelty implications:**
- This is the companion to park2024enhancingb (K-City).
- It shows the group's checks are CaTch/ETSI-style, not SM-MBR.
- It treats slander as malicious (citing 5GAA 2022) and does not measure benign trigger rates of any standard rule set.
