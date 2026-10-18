# Methodologies of Link-Level Simulator and System-Level Simulator for C-V2X Communication
Donglin Wang, Raja R. Sattiraju, Anjie Qiu, Sanket Partani, Hans D. Schotten · 2019 IEEE 2nd International Conference on Electronics and Communication Engineering (ICECE) 2019 · https://doi.org/10.1109/icece48499.2019.9058545 · relevance: high · basis: full text

**Problem:** Need both link-level and system-level simulators for sidelink C-V2X.
**Approach:** Implements LL simulator producing BLER-SNR curves used as link-to-system interface for an SL simulator with multiple eNBs and UEs.
**Tools and scale:** Own LTE E-UTRA standard-compliant LL and SL simulators.
**Results:** LL and SL meet different objectives (qualitative).
**Limitations and future work (as stated by the authors):**
- Without computation limits a single tool could do both
- realistically they are separate.
**Overlap with our engine:** partial — L2S abstraction used in our PHY.
**Novelty implications:** No impact.
