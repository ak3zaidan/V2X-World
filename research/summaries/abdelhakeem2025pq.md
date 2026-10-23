# PQ-V2X: A Novel Postquantum Cryptographic Dataset for Secure Vehicular Communications
Shimaa Abdelnaby AbdelHakeem, HyungWon Kim · IEEE Internet of Things Journal 2025 · https://doi.org/10.1109/jiot.2025.3618153 · relevance: high · basis: released generator code and dataset pages (the paper is paywalled)

**Problem:** No V2X intrusion-detection dataset that combines PQ crypto artefacts, mobility and channel.
**Approach (from `generation/generate_v1_paper.py`, GitHub ShimaaAbdelHakeem/PQ-V2X-Dataset):**
- Each of the 5 M rows is an independent i.i.d. draw: uniform latitude/longitude over a San Francisco box, normal speed, a random zone, uniform SNR and loss per zone. There are no trajectories, messages, network or time series.
- Attack features are drawn conditional on the label (e.g. Shor → `sig_generation_time` U(200,400); DDoS → frequency 1,500–5,000); "adversarial" noise is applied only to Grover/Shor/QuantumNoise rows.
- `risk_score` and `synthetic_anomaly_score` each include 0.25·`is_attack`.
- The "real traces" (≈10%) are liboqs keypairs and signatures over the constant string `b"V2X_SECURE_MESSAGE"`.
**Tools and scale:** v1: 55 columns, 24 classes (DataPort 10.21227/830s-zh96); v2: 65 columns (10.21227/6qq2-9s87). Both need a DataPort subscription.
**Results:** The paper reports "LightGBM attains perfect detection" and "tunnel zones amplify risk". Both follow from the generator: the label is written into features, and the tunnel SNR profile feeds `risk_score`.
**Limitations and future work (as stated by the authors):**
- README: `is_real_trace` "should not be described as a field-collected vehicle trajectory or field-measured V2X network trace".
**Overlap with our engine:** weak in substance, despite the label "PQ V2X dataset": there is no executed protocol and no PKI event.
**Novelty implications:**
- An independent leakage-aware re-evaluation (GitHub marcosFigueredo/pqc-v2x-hierarchical-detection) finds "duplicate-pattern leakage": random-split macro-F1 is above 0.98, but grouped-split LightGBM macro-F1 is only 0.35–0.59.
- Cite it as a cautionary example, not as a peer benchmark. See synthesis/check-datasets-2026.md §1.9.
