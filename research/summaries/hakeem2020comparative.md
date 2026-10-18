# Comparative Experiments of V2X Security Protocol Based on Hash Chain Cryptography
Shimaa A. Abdel Hakeem, Mohamed A. Abd El-Gawad, Hyungwon Kim · Italian National Conference on Sensors 2020 · https://doi.org/10.3390/s20195719 · relevance: high · basis: full text

**Problem:** ECDSA-based V2X security is too costly.
**Approach:** Hash-chain MAC authentication with pre-generated pseudo-identities, compared with IEEE 1609.2, ETSI 103 097, 2FLIP, TESLA.
**Tools and scale:** Real V2X network with commercial V2X devices.
**Results:** 40-byte overhead (up to 6x smaller); 25,000 BSM/s verification in software vs 163 (1609.2) and 183 (ETSI) msg/s with full certificate on HSM, 35-36 msg/s in software.
**Limitations and future work (as stated by the authors):**
- None stated.
**Overlap with our engine:** partial — Measures 1609.2 verification throughput on real devices; alternative non-PKI scheme.
**Novelty implications:** Real-device 1609.2 verification rates exist as baselines for verification-load studies.
