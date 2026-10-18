# Bridging the Gap Between Simulated and Real Network Data Using Transfer Learning
Carlos Güemes-Palau, Miquel Ferriol-Galmés, Jordi Paillisse-Vilanova, Albert López-Brescó, Pere Barlet-Ros, Albert Cabellos-Aparicio · arXiv 2025 · https://doi.org/10.1109/netsoft70012.2026.11603556 · relevance: high · basis: full text (extracted sections)

**Problem:** Real network data is scarce; models trained on simulation lose accuracy in reality.
**Approach:** Transfer learning: pretrain RouteNet-Fermi on OMNeT++ data, fine-tune on small real testbed data.
**Tools and scale:** OMNeT++; custom testbed with MGEN/Tcpreplay.
**Results:** Delay MAPE reduced up to 88% (Poisson), 80% (On/Off); 37% with 10 and 48% with 50 real scenarios.
**Limitations and future work (as stated by the authors):**
- None stated in extracted sections.
**Overlap with our engine:** partial - sim-to-real surrogate transfer (direction 3).
**Novelty implications:** No effect on V2X.
