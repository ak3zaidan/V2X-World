# BMuSig2: Schnorr-Compatible Blind Multi-Signatures
Kanchan Bisht, Keerthi Aiswarya Varshini, Shivam Sethi, Maria Francis et al. · Proceedings of the ACM Asia Conference on Computer and Communications Security 2026 · https://doi.org/10.1145/3779208.3805986 · relevance: low · basis: abstract

**Problem:** Blind signatures and multi-signatures are well-known primitives, but blind multi-signatures (BMS), which combine both these primitives, were only recently formalized by Karantaidou et al. (CCS'24). A BMS scheme allows a user to obtain a compact signature on a common hidden message from a group of signers such that even if the signers collude, they cannot learn the message or ...
**Approach:** In this paper, we introduce BMuSig2, a 2-round concurrently secure blind multi-signature scheme whose signatures and verification match standard Schnorr signatures. This design enables systems using Schnorr signatures to adopt BMuSig2 as a drop-in replacement, requiring changes only to the issuance phase, while leaving verification unchanged.
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** We also provide a proof-of-concept implementation to demonstrate its practical efficiency.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: threshold/multi-party signatures (general, non-vehicular). Our threshold authorities (FROST, CGGMP21) build on this family of schemes; nothing here applies them to an SCMS.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
