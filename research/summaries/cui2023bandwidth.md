# Bandwidth-Efficient Zero-Knowledge Proofs For Threshold ECDSA
Handong Cui, Kwan Yin Chan, Tsz Hon Yuen, Xin Kang et al. · The Computer Journal 2023 · https://doi.org/10.1093/comjnl/bxad057 · relevance: low · basis: abstract

**Problem:** In most threshold Elliptic Curve Digital Signature Algorithm (ECDSA) signatures using additively homomorphic encryption, the zero-knowledge (ZK) proofs related to the ciphertext or the message space are the bottleneck in terms of bandwidth as well as computation time.
**Approach:** In this paper, we propose a compact ZK proof for relations related to the Castagnos–Laguillaumie (CL) encryption, which is 33% shorter and 29% faster than the existing work in PKC 2021. We also give new ZK proofs for relations related to homomorphic operations over the CL ciphertext.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** These new ZK proofs are useful to construct a bandwidth-efficient universal composable-secure threshold ECDSA without compromising the proactive security and the non-interactivity. In particular, we lowered the communication and computation cost of the key refresh algorithm in the Paillier-based counterpart from $O(n^3)$ to $O(n^2)$.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — threshold/distributed signing or CA background relevant to the engine's threshold authorities (FROST, CGGMP21, Umbrella); no V2X simulation or SCMS integration stated in the abstract.
**Novelty implications:** Does not rule out simulation-level claims; it is cryptographic background. Any claim of novelty in the threshold protocol itself would need to be checked against this work.
