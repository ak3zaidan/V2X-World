# Threshold ECDSA in Two Rounds
Yingjie Lyu, Zengpeng Li, Hong-Sheng Zhou, Xudong Deng · Proceedings of the 2025 ACM SIGSAC Conference on Computer and Communications Security 2025 · https://doi.org/10.1145/3719027.3765176 · relevance: low · basis: abstract

**Problem:** We also resolve the security issue of presigning pointed out by Groth and Shoup (Eurocrypt '22), evading a security loss that increases with the number of pre-released, unused presignatures, for the first time among threshold-optimal schemes.
**Approach:** Our construction builds on Non-Interactive Multiplication (NIM), a notion proposed by Boyle et al. (PKC '25), which allows parties to evaluate multiplications on secret-shared values in one round. In particular, we use the construction of Abram et al. (Eurocrypt '24) instantiated with class groups.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** The signing protocol is efficient in bandwidth, with a message size of 1.9 KiB at 128-bit security, and has competitive computational performance.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — threshold/distributed signing or CA background relevant to the engine's threshold authorities (FROST, CGGMP21, Umbrella); no V2X simulation or SCMS integration stated in the abstract.
**Novelty implications:** Does not rule out simulation-level claims; it is cryptographic background. Any claim of novelty in the threshold protocol itself would need to be checked against this work.
