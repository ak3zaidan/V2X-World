# Threshold ECDSA in Three Rounds
Jack Doerner, Yashvanth Kondi, Eysa Lee, Abhi Shelat · 2024 IEEE Symposium on Security and Privacy (SP) 2024 · https://doi.org/10.1109/sp54263.2024.00178 · relevance: low · basis: abstract

**Problem:** We present a three-round protocol for threshold ECDSA signing with malicious security against a dishonest majority, which information-theoretically UC-realizes a standard threshold signing functionality, assuming only ideal commitment and two-party multiplication primitives.
**Approach:** Our protocol combines an intermediate representation of ECDSA signatures that was recently introduced by Abram et al. [2] with an efficient statistical consistency check reminiscent of the ones used by the protocols of Doerner et al. [3], [4]. We show that shared keys for our signing protocol can be generated using a simple commit-release-and-complain procedure, without any proofs of knowledge, and to compute the intermediate representation of each signature, we propose a two-round vectorized multiplication protocol based on oblivious transfer that outperforms all similar constructions.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** We demonstrate empirically that our protocol outperforms those of Doerner et al. by factors of as much as six in high-latency environments, and that it is multiple orders of magnitude faster than Paillier-based approaches.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — threshold/distributed signing or CA background relevant to the engine's threshold authorities (FROST, CGGMP21, Umbrella); no V2X simulation or SCMS integration stated in the abstract.
**Novelty implications:** Does not rule out simulation-level claims; it is cryptographic background. Any claim of novelty in the threshold protocol itself would need to be checked against this work.
