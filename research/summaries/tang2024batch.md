# Batch Range Proof: How to Make Threshold ECDSA More Efficient
Guofeng Tang, Shuai Han, Li Lin, Changzheng Wei et al. · Proceedings of the 2024 on ACM SIGSAC Conference on Computer and Communications Security 2024 · https://doi.org/10.1145/3658644.3670287 · relevance: low · basis: abstract

**Problem:** However, the overhead of range proofs constitutes a major portion of the total cost.
**Approach:** So far, several methods have been proposed to construct threshold ECDSA, including the usage of OT and homomorphic encryptions (HE). In this paper, we propose efficient batch range proofs to improve the efficiency of threshold ECDSA.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** At the heart of our efficiency improvement is a new technical tool calledMulti-Dimension Forking Lemma, as a generalization of the well-known general forking lemma [Bellare and Neven, CCS 2006]. Based on our new tool, we construct efficient batch range proofs for Paillier and JL encryptions, and use them to give batch multiplication-to-addition (MtA) protocols, which are crucial to most threshold ECDSA.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — threshold/distributed signing or CA background relevant to the engine's threshold authorities (FROST, CGGMP21, Umbrella); no V2X simulation or SCMS integration stated in the abstract.
**Novelty implications:** Does not rule out simulation-level claims; it is cryptographic background. Any claim of novelty in the threshold protocol itself would need to be checked against this work.
