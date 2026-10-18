# VeMisNet: Enhanced Feature Engineering for Deep Learning-Based Misbehavior Detection in Vehicular Ad Hoc Networks
Nayera Youness, Ahmad Mostafa, Mohamed A. Sobh, Ayman M. Bahaa, Khaled Nagaty · Journal of Sensor and Actuator Networks 2025 · https://doi.org/10.3390/jsan14050100 · relevance: high · basis: full text

**Problem:** Feature engineering and scalability for DL MBD.
**Approach:** Domain-informed spatiotemporal features (DSRC neighbourhood density, inter-message timing, frequency); LSTM/GRU/BiLSTM from 100K to 2M samples; SMOTE after split.
**Tools and scale:** VeReMi extension (LuST), all 20 attack categories.
**Results:** BiLSTM 99.81% accuracy and F1 at 500K; >99.5% at 2M; 0.19% missed attacks, <0.05% false alarms, 41.76 ms inference; 27.5% gain over dimensionality reduction.
**Limitations and future work (as stated by the authors):**
- Reliance on simulated data (LuST only)
- single 80/20 split, no k-fold or temporal CV
- potential adversarial vulnerability
- future: end-to-end deployment study with real-time throughput and latency.
**Overlap with our engine:** partial — offline; explicitly calls for end-to-end deployment evaluation.
**Novelty implications:** Supports open need for in-loop, multi-scenario evaluation.
