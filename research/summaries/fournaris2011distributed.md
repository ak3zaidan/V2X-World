# DISTRIBUTED THRESHOLD CRYPTOGRAPHY CERTIFICATION WITH NO TRUSTED DEALER
A. Fournaris · Proceedings of the International Conference on Security and Cryptography 2011 · https://doi.org/10.5220/0003525304000404 · relevance: low · basis: abstract

**Problem:** However, existing threshold cryptography distributed key generation and certification systems still rely on a single, centralized, trusted entity at some point during the certification process (usually during initialization) to split the secret and distribute it to all distributed system participants.
**Approach:** In this paper, we deal with this problem by extending the a key generation scheme of Noack and Spitz (2009) and by proposing a certification scheme that has no need for a trusted dealer to create, split and distribute the proposed certification scheme's private-public key pair. The proposed scheme uses the participant addition-removal procedure described in (Noack and Spitz, 2009) that does not affect the scheme's public key (used for certificate verification) and has small interference to the certification process as a whole.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** To reduce the computational cost the proposed system employs Elliptic Curve Cryptography (ECC) principles.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — threshold/distributed signing or CA background relevant to the engine's threshold authorities (FROST, CGGMP21, Umbrella); no V2X simulation or SCMS integration stated in the abstract.
**Novelty implications:** Does not rule out simulation-level claims; it is cryptographic background. Any claim of novelty in the threshold protocol itself would need to be checked against this work.
