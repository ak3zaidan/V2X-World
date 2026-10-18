# Lattice-Based Threshold Signature Implementation for Constrained Devices
Patrik Dobias, Sara Ricci, Petr Dzurenda, Lukas Malina et al. · Proceedings of the 20th International Conference on Security and Cryptography 2023 · https://doi.org/10.5220/0012112700003555 · relevance: low · basis: abstract

**Problem:** : Threshold signatures have gained increased attention especially due to their recent applications in blockchain technologies.
**Approach:** In this paper, we propose the optimization of the DS2 lattice-based ( n , n ) -threshold signature scheme and present its practical implementation. Moreover, we evaluate our optimized implementation of the DS2 scheme on different platforms.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** Even if the Schnorr-based threshold signatures improve the blockchain’s privacy and scalability, these schemes do not provide post-quantum security. The results demonstrate that our implementation is easily portable and executable on constrained devices based on ARM Cortex-A53, ARM Cortex-M3, and ESP32 architectures.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — threshold/distributed signing or CA background relevant to the engine's threshold authorities (FROST, CGGMP21, Umbrella); no V2X simulation or SCMS integration stated in the abstract.
**Novelty implications:** Does not rule out simulation-level claims; it is cryptographic background. Any claim of novelty in the threshold protocol itself would need to be checked against this work.
