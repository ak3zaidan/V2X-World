# Efficient Multiplicative-to-Additive Function from Joye-Libert Cryptosystem and Its Application to Threshold ECDSA
Haiyang Xue, Man Ho Au, Mengling Liu, Kwan Yin Chan et al. · Proceedings of the 2023 ACM SIGSAC Conference on Computer and Communications Security 2023 · https://doi.org/10.1145/3576915.3616595 · relevance: low · basis: abstract

**Problem:** Threshold ECDSA receives interest lately due to its widespread adoption in blockchain applications.
**Approach:** In this paper, we design a novel MtA by revisiting the Joye-Libert (JL) cryptosystem. Specifically, we revisit JL encryption and propose a JL-based commitment, then give efficient zero-knowledge proofs for JL cryptosystem which are the first to have standard soundness.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** Specifically, O(n2) invocations of MtA are required in the case of n active signers. Hence, improvement of MtA leads directly to significant improvements for all state-of-the-art threshold ECDSA schemes.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — threshold/distributed signing or CA background relevant to the engine's threshold authorities (FROST, CGGMP21, Umbrella); no V2X simulation or SCMS integration stated in the abstract.
**Novelty implications:** Does not rule out simulation-level claims; it is cryptographic background. Any claim of novelty in the threshold protocol itself would need to be checked against this work.
