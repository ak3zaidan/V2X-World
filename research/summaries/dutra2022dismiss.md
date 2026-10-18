# DISMISS-BSM: an Architecture for Detecting Position Spoofing in Basic Safety Messages
Fernando Dutra, Kenniston Bonfim, Carlos Siqueira, L. A. Pereira, Aldri L. Santos, Rodolfo Ipolito Meneguette · Global Communications Conference 2022 · https://doi.org/10.1109/globecom48099.2022.10001122 · relevance: high · basis: abstract

**Problem:** BSM position forgery.
**Approach:** DISMISS-BSM: buffered received messages as predictors with RSSI and movement-pattern disruption features; sliding windows 2 to 23.
**Tools and scale:** Compared K-NN, DT, MLP, LSTM.
**Results:** Decision trees best; ~47 s training; outperforms state of the art.
**Limitations and future work (as stated by the authors):**
- None stated.
**Overlap with our engine:** partial — RSSI-plus-kinematics MBD.
**Novelty implications:** None.
