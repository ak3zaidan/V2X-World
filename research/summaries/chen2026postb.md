# Post-Quantum Cryptography Key Expansion Method and Anonymous Certificate Scheme Based on NTRU
Abel C. H. Chen · arXiv.org 2026 · https://arxiv.org/abs/2601.07841 · relevance: low · basis: abstract

**Problem:** NTRU is one of the important lattice-based post-quantum cryptography methods, offering resistance against quantum computing attacks. However, a drawback of NTRU lies in its relatively low efficiency in generating key pairs.
**Approach:** Therefore, this study proposes an NTRU-based key expansion method that enables efficient public key expansion. Furthermore, the proposed method is applied to an anonymous certificate scheme, allowing an end entity to generate a key pair only once, after which the certificate authority can expand multiple distinct public keys for anonymity.
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** The experimental results demonstrate that the proposed key expansion method achieves significantly higher efficiency than key pair generation.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
