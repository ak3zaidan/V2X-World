# Design-Space Mapping of Post-Quantum Cryptographic Artifact Transport on CAN-FD: A Discrete-Event Simulation Study
Minwoo Lee, Minjoo Sim, Siwoo Eum, Gyeongju Song et al. · Applied Sciences 2026 · https://doi.org/10.3390/app16083705 · relevance: high · basis: full text

**Problem:** PQC artifacts must be segmented over CAN-FD alongside safety traffic; feasibility unmapped.
**Approach:** Validated discrete-event simulator of CAN-FD arbitration and ISO-TP, evaluating 29 parameter sets (12 KEM, 17 DSA) from nine families across 534 scenarios.
**Tools and scale:** In-vehicle CAN-FD DES.
**Results:** KEM feasibility depends on domain scale and startup coordination more than algorithm; 4-ECU staggered feasible for Level-1 candidates; 16-ECU simultaneous infeasible; FN-DSA best signature; HQC infeasible, SLH-DSA nearly so; traffic shaping beats algorithm substitution.
**Limitations and future work (as stated by the authors):**
- Deterministic frame timing, no errors or retransmissions (underestimates tails).
- Fixed ISO-TP parameters; single bus segment; ideal arbitration.
- Future work: real-vehicle CAN-FD traces, HIL validation, multi-bus and CAN-FD/Ethernet topologies.
**Overlap with our engine:** none — in-vehicle bus, not V2X; methodologically similar design-space mapping.
**Novelty implications:** Analogue methodology for in-vehicle; V2X design-space mapping remains our space.
