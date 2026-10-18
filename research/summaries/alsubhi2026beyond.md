# Beyond Random-Split Accuracy: Duplicate-Safe and Crypto-Agile Evaluation of Anomaly Detection for Post-Quantum TLS
Mohammed Abdulaziz Alsubhi · Electronics 2026 · https://doi.org/10.3390/electronics15184179 · relevance: low · basis: abstract

**Problem:** Post-quantum cryptography changes the size, timing, and algorithmic context of Transport Layer Security (TLS) handshakes, creating a dynamic normal class for anomaly detectors. This study evaluates an assurance framework, rather than proposing a new classifier, on CIC-PQC_OAV v1: 40,010 sessions represented by 32 encrypted-metadata features.
**Approach:** This study evaluates an assurance framework, rather than proposing a new classifier, on CIC-PQC_OAV v1: 40,010 sessions represented by 32 encrypted-metadata features. We compare the fixed split with five-seed, size-matched sample-stratified, exact-disjoint, raw round-3-disjoint, and IQR-normalized round-3-disjoint protocols, each separating fitting, probability calibration, threshold selection, conformal ...
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** Isotonic calibration yields a Brier score of 0.0221±0.0013, while five-seed perturbations confirm sensitivity to timing masking and byte scaling.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: threshold/multi-party signatures (general, non-vehicular). Our threshold authorities (FROST, CGGMP21) build on this family of schemes; nothing here applies them to an SCMS.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
