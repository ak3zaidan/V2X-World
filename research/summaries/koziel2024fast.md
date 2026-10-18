# Fast Two-party Threshold ECDSA with Proactive Security
Brian Koziel, S. Dov Gordon, Craig Gentry · Proceedings of the 2024 on ACM SIGSAC Conference on Computer and Communications Security 2024 · https://doi.org/10.1145/3658644.3670387 · relevance: low · basis: abstract

**Problem:** However, the scheme pushes that complexity into key generation.
**Approach:** We overcome all of these issues with a proactive Refresh procedure. Since the Paillier decryption key is part of the secret that must be proactively refreshed, our first improvement is to radically accelerate key generation by replacing one of Lindell's ZK proofs -- which requires 80 Paillier ciphertexts for statistical security 2-40 -- with a much faster "weak" proof that requires only 2 Paillier ciphertexts, and which proves a weaker statement about a Paillier ciphertext that we show is sufficient in the context of our scheme.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** Secondly, our more efficient key generation procedure also makes frequent proactive Refreshes practical. Finally, we show that adding noise to one party's key share suffices to avoid the need to reset the public verification key when certain bad behavior is detected.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — threshold/distributed signing or CA background relevant to the engine's threshold authorities (FROST, CGGMP21, Umbrella); no V2X simulation or SCMS integration stated in the abstract.
**Novelty implications:** Does not rule out simulation-level claims; it is cryptographic background. Any claim of novelty in the threshold protocol itself would need to be checked against this work.
