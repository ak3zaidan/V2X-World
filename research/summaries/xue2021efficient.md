# Efficient Online-friendly Two-Party ECDSA Signature
Haiyang Xue, M. Au, Xiang Xie, Tsz-Hon Yuen et al. · Conference on Computer and Communications Security 2021 · https://doi.org/10.1145/3460120.3484803 · relevance: low · basis: abstract

**Problem:** However, the existing two-party protocols of ECDSA are not optimal: either their online phase requires decryption of a ciphertext, or their offline phase needs at least two executions of multiplicative-to-additive conversion which dominates the overall complexity.
**Approach:** Depending on whether or not the message is required, we could divide two-party signing into two different phases, namely, offline and online. This paper proposes an online-friendly two-party ECDSA with a lightweight online phase and a single multiplicative-to-additive function in the offline phase.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** Our scheme significantly improves previous protocols based on either oblivious transfer or homomorphic encryption. We implement our scheme and show that it outperforms prior online-friendly schemes (i.e., those have lightweight online cost) by a factor of roughly 2 to 9 in both communication and computation.
**Limitations and future work (as stated by the authors):**
- "At the same time, the cost of the offline phase should remain similar to that of a normal signature generation."
**Overlap with our engine:** partial — threshold/distributed signing or CA background relevant to the engine's threshold authorities (FROST, CGGMP21, Umbrella); no V2X simulation or SCMS integration stated in the abstract.
**Novelty implications:** Does not rule out simulation-level claims; it is cryptographic background. Any claim of novelty in the threshold protocol itself would need to be checked against this work.
