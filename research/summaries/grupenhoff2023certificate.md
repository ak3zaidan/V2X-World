# Certificate-Based Key Replacement in Single Use Hash-Based Signatures: A Post-Quantum Technique
Matthew D. Grupenhoff · International Journal of High School Research 2023 · https://doi.org/10.36838/v5i4.14 · relevance: low · basis: abstract

**Problem:** : The recent advancement in quantum computing poses a concern over the security of current public key cryptography systems.
**Approach:** This paper discusses a solution to these two problems through key replacement to allow a single initial agreement to be used an arbitrary number of times with a reasonable key size.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** The experiments were done by extending the Lamport signature scheme with this key replacement, leading to a signature scheme that is slower than DSA and ECDSA, but faster than RSA for sufficiently large security levels.
**Limitations and future work (as stated by the authors):**
- "Since then, hash-based signature schemes have been created to replace vulnerable schemes, although they have limitations such as requiring a large key and/or signature size or only working a limited number of times."
**Overlap with our engine:** partial — threshold/distributed signing or CA background relevant to the engine's threshold authorities (FROST, CGGMP21, Umbrella); no V2X simulation or SCMS integration stated in the abstract.
**Novelty implications:** Does not rule out simulation-level claims; it is cryptographic background. Any claim of novelty in the threshold protocol itself would need to be checked against this work.
