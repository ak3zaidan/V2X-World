# Time-Series Foundation Models for ISP Traffic Forecasting
Fan Liu, Behrooz Farkiani, Patrick Crowley · NOMS 2026-2026 IEEE Network Operations and Management Symposium 2026 · https://doi.org/10.1109/noms69089.2026.11668147 · relevance: medium · basis: abstract

**Problem:** Accurate network-traffic forecasting enables proactive capacity planning and anomaly detection in Internet Service Provider (ISP) networks. Recent advances in time-series foundation models (TSFMs) have demonstrated strong zero-shot and few-shot generalization across diverse domains, yet their effectiveness for computer networking remains unexplored.
**Approach:** This paper presents a systematic evaluation of a TSFM, IBM's Tiny Time Mixer (TTM), on the CESNET-TimeSeries24 dataset, a 40-week real-world ISP telemetry corpus.
**Tools and scale:** Not stated in the abstract.
**Results:** Results show that TTM achieves consistent accuracy (RMSE 0.026-0.057) and stable $R^2$ scores across horizons and context lengths, outperforming or matching fully trained deep learning baselines such as GRU and LSTM. Inference latency remains under 0.05s per 100 points on a single MacBook Pro using CPU-only computation, confirming deployability without dedicated GPU or MPS acceleration.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — Learned wireless/PHY model (foundation or world model); no V2X security, PKI or closed-loop traffic.
**Novelty implications:** Shows learned radio surrogates are active work; a surrogate trained on joint traffic-radio-PKI ground truth with counterfactual labels is not addressed.
