# 5G NR-V2X Scheduling Approaches for CPM Variable Size Traffic
Vittorio Todisco, Mattia Andreani, M. L. Merani, Alessandro Bazzi · IEEE Vehicular Networking Conference 2026 · https://doi.org/10.1109/vnc69225.2026.11629139 · relevance: high · basis: full text

**Problem:** Variable-size CPMs (including periodic security overhead) disturb NR-V2X Mode 2 semi-persistent scheduling.
**Approach:** System-level comparison of dynamic scheduling and SPS variants (padding, size-driven reselection, MCS adaptation) with CPM traffic reconstructed from real perception datasets.
**Tools and scale:** NR-V2X Mode 2 system-level simulation; real perception datasets.
**Results:** Dynamic scheduling and aggressive reselection raise collisions; SPS with padding is stable and robust; MCS adaptation gives the highest average PRR but uneven reliability; stability matters more than instantaneous load optimisation.
**Limitations and future work (as stated by the authors):**
- Not explicitly stated in extracted text.
**Overlap with our engine:** partial — security-overhead-induced size variability in NR-V2X Mode 2, related to PQ certificate size effects in direction 2 and our NR-V2X stack.
**Novelty implications:** Size-variability effects on SPS are documented for CPMs; open: the same with PQ/hybrid certificate digest-vs-full-certificate switching.
