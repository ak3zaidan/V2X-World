# Nonce generation techniques in Schnorr multi-signatures: Exploring EdDSA-inspired approaches
Nawras H. Sabbry, Alla Levina · AIMS Mathematics 2024 · https://doi.org/10.3934/math.2024988 · relevance: low · basis: abstract

**Problem:** This paper proposes a deterministic nonce generation technique to address the catastrophic issues associated with nonce reuse in message signing and to enhance the efficiency of Schnorr multi-signature schemes.
**Approach:** The proposed method was inspired by the EdDSA approach. This analysis evaluates the effectiveness and robustness of the proposed nonce generation technique within the frameworks of the Schnorr digital signature and the two-round MuSig schemes.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** Additionally, this research aims to reduce computational complexity and bandwidth requirements in digital and multi-signature schemes while maintaining robust security against common attacks. Furthermore, it is proposed to exclude the public nonce R from the challenge calculations and to allow signers to directly prove possession of their secret keys through the aggregated public key, thereby eliminating the need for non-interactive zero-knowledge (NIZK) proofs.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — a standalone authentication/crypto scheme evaluated in isolation (per the abstract); no closed-loop traffic, radio and SCMS co-simulation.
**Novelty implications:** Leaves open how such schemes behave inside a full SCMS with realistic traffic and radio load; it does not rule out any candidate direction.
