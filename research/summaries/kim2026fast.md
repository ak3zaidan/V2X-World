# Fast Two-Party ECDSA With Online-Efficient Signing
Sungwook Kim, Yulim Shin, Hyung Tae Lee · IEEE Access 2026 · https://doi.org/10.1109/access.2026.3676989 · relevance: low · basis: abstract

**Problem:** Two-party ECDSA enables two participants, each holding a share of the signing key, to collaboratively generate an ECDSA signature. With the rise of the blockchain technology, it has received significant attention as a method to enhance the security of digital assets.
**Approach:** In CRYPTO 2017, Lindell proposed an efficient two-party ECDSA protocol based on additively homomorphic encryption (AHE). In this paper, we propose a new two-party ECDSA scheme inspired by Lindell’s protocol.
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** The experimental results demonstrate that: 1) our protocol completes the online signing phase in only 0.07 ms, whereas Lindell’s protocol requires 22.30 ms, and 2) both protocols require approximately 23 ms in total, including both the offline and online phases.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: threshold/multi-party signatures (general, non-vehicular). Our threshold authorities (FROST, CGGMP21) build on this family of schemes; nothing here applies them to an SCMS.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
