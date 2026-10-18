# Feasibility and Benchmarking of Post-Quantum Cryptography in the Cooperative ITS Ecosystem
Brigitte Lonc, Alexandre Aubry, Hafeda Bakhti, Maria Christofi et al. · IEEE Vehicular Networking Conference (VNC) 2023 · https://doi.org/10.1109/vnc57357.2023.10136335 · relevance: high · basis: full text

**Problem:** Can NIST PQC algorithms replace ECC in C-ITS under latency and bandwidth constraints?
**Approach:** Defines C-ITS requirements and KPIs and benchmarks NIST PQC candidates (TAM project).
**Tools and scale:** Benchmarks on C-ITS-relevant hardware.
**Results:** No algorithm can simply replace C-ITS algorithms on current hardware; Falcon and Kyber chosen for transition; hybrid Falcon+ECDSA the only backward-compatible approach with reasonable impact.
**Limitations and future work (as stated by the authors):**
- Standards need modification and some assumptions relaxed
- KEM integration in PKI protocols needs more study
- plan to focus on hybrid Falcon+ECDSA
- awaiting optimised algorithms and hardware accelerators
**Overlap with our engine:** partial — PQ feasibility for C-ITS messages and PKI; hardware benchmarks without network/traffic simulation.
**Novelty implications:** Qualitative feasibility verdict exists; quantified safety cost in closed-loop simulation remains.
