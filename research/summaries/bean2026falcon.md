# FALCON-C: Flow-Based Analysis and Labeling for Connected Vehicular Network Cybersecurity
Joshua Bean, Dimitrios Michael Manias · 2026 IEEE 27th International Conference on High Performance Switching and Routing (HPSR) 2026 · https://doi.org/10.1109/hpsr68369.2026.11615195 · relevance: high · basis: full text

**Problem:** EV charging equipment communication is a cyber target and SOC flow labelling is manual.
**Approach:** FALCON-C: an autoencoder trained on benign flows from CICEVSE2024 labels malicious flows by reconstruction error, with a refined decision boundary.
**Tools and scale:** CICEVSE2024 dataset.
**Results:** 100% detection of malicious flows; refined boundary improves performance by 8.6% after initial false positives.
**Limitations and future work (as stated by the authors):**
- False-positive flows need deeper study of their communication patterns.
- Future work: more features, other autoencoder architectures and an ensemble.
**Overlap with our engine:** none — EV charging network IDS, not V2X.
**Novelty implications:** No impact.
