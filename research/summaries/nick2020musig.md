# MuSig-DN: Schnorr Multi-Signatures with Verifiably Deterministic Nonces
Jonas Nick, Tim Ruffing, Yannick Seurin et al. · Proceedings of the 2020 ACM SIGSAC Conference on Computer and Communications Security 2020 · https://doi.org/10.1145/3372297.3417236 · relevance: low · basis: abstract

**Problem:** MuSig is a multi-signature scheme for Schnorr signatures, which supports key aggregation and is secure in the plain public key model.
**Approach:** In this paper, we propose a variant of MuSig in which signers generate their nonce deterministically as a pseudorandom function of the message and all signers' public keys and prove that they did so by providing a non-interactive zero-knowledge proof to their cosigners. The resulting scheme, which we call MuSig-DN, is the first Schnorr multi-signature scheme with deterministic signing.
**Tools and scale:** Not stated in the abstract.
**Results:** No quantitative results stated in the abstract.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Studies V2X attacks or their detection; overlaps the misbehaviour/attack modules but without linkage-based revocation, closed loop to the SCMS, or joint ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
