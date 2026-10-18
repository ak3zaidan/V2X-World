# Adaptive Quantum-Safe Cryptography for 6G Vehicular Networks via Context-Aware Optimization
Poushali Sengupta, Mayank Raikwar, Sabita Maharjan, Frank Eliassen et al. · NDSS 2026 - FutureG Workshop 2026 · https://doi.org/10.14722/futureg.2026.240099 · relevance: high · basis: full text

**Problem:** PQC overhead conflicts with 6G V2X latency; switching algorithms opens downgrade attacks.
**Approach:** CAAP: predicts mobility and channel variation and selects lattice/code/hash PQC configurations via a predictive multi-objective evolutionary algorithm (APMOEA), with a secure monotonic-upgrade protocol against downgrade, replay and desynchronisation.
**Tools and scale:** LuST mobility traces, ERA5 weather, NR-V2X channel models, automotive compute profiles; trace-based, not full-stack.
**Results:** Latency reduced up to 27%; downgrade attempts rejected; robust to context manipulation and prediction noise.
**Limitations and future work (as stated by the authors):**
- Relies on clock synchronisation and prediction accuracy.
- No live deployment validation; trace-based modelling only.
- Future work: formal verification of RL-assisted adaptation, hardware-in-the-loop testbeds.
- Future work: jointly optimise encryption and signature schemes.
**Overlap with our engine:** partial — PQ algorithm selection under NR-V2X latency constraints relates to direction 2, but no PKI backend or full-stack radio.
**Novelty implications:** Adaptive PQ selection exists; open: crypto-agility evaluated end to end with SCMS issuance and real radio stacks.
