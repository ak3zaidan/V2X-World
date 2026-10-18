# SPlatoon: Secure Platoon with Lightweight Authenticated Encryption
G. Mandal, Ipsita Koley, Ayantika Chatterjee, Soumyajit Dey · ACM Transactions on Cyber-Physical Systems 2026 · https://doi.org/10.1145/3827604 · relevance: high · basis: abstract

**Problem:** Authenticated encryption of CAMs increases size and channel load, delaying platoon messages.
**Approach:** Reduce CAM payload numerical precision before AE encryption and assess platoon safety. (summarised from the abstract only)
**Tools and scale:** Platoon simulation (details not in abstract).
**Results:** Platooning remains safe at certain reduced precisions; arbitrary reduction causes control errors that propagate and may compromise safety.
**Limitations and future work (as stated by the authors):**
- Not stated in abstract.
**Overlap with our engine:** partial — trades security overhead against platoon safety, a concrete instance of direction 2.
**Novelty implications:** Security-overhead vs safety trade-off is shown for platoon CAM encryption; open: city-scale, signature/PQ overhead and SCMS.
