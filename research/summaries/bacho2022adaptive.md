# On the Adaptive Security of the Threshold BLS Signature Scheme
Renas Bacho, J. Loss · IACR Cryptology ePrint Archive 2022 · https://doi.org/10.1145/3548606.3560656 · relevance: low · basis: abstract

**Problem:** Threshold signatures are a crucial tool for many distributed protocols.
**Approach:** In this work, we revisit the security of the threshold BLS signature by showing the following results, assuming t adaptive corruptions: - We give a modular security proof that follows a two-step approach: 1) We introduce a new security notion for distributed key generation protocols (DKG). We show that it is satisfied by several protocols that previously only had a static security proof. 2) Assuming any DKG protocol with this property, we then prove unforgeability of the threshold BLS scheme.
**Tools and scale:** Not stated in the abstract.
**Results:** Our reductions are tight and can be used to substantiate real-world parameter choices. - To justify our use of strong assumptions such as the algebraic group model (AGM) and the hardness of one-more-discrete logarithm (OMDL), we prove an impossibility result: Even in the AGM, a strong interactive assumption is required in order to prove the scheme secure.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Security/authentication scheme for vehicular networks; overlaps the 1609.2/SCMS layer in topic but not the US SCMS design (butterfly keys, LA/MA, linkage-based CRL) or simulation-integrated evaluation.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
