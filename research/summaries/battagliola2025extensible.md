# Extensible decentralized secret sharing and application to Schnorr signatures
Michele Battagliola, Riccardo Longo, Alessio Meneghetti · Designs, Codes and Cryptography 2025 · https://doi.org/10.1007/s10623-025-01746-1 · relevance: low · basis: abstract

**Problem:** Starting from links between Coding Theory and Secret Sharing Schemes, we develop an extensible and decentralized version of Shamir Secret Sharing, that allows the addition of new users after the initial share distribution.
**Approach:** On top of it we design a totally decentralized ( t , n )-threshold Schnorr signature scheme that needs only t users online during the key generation phase, while the others join later. Under standard assumptions we prove our scheme secure against adaptive malicious adversaries.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** Using a classical game-based argument, we prove that if there is an adversary capable of forging the scheme with non-negligible probability, then we can build a forger for the centralized Schnorr scheme with non-negligible probability.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — threshold/distributed signing or CA background relevant to the engine's threshold authorities (FROST, CGGMP21, Umbrella); no V2X simulation or SCMS integration stated in the abstract.
**Novelty implications:** Does not rule out simulation-level claims; it is cryptographic background. Any claim of novelty in the threshold protocol itself would need to be checked against this work.
