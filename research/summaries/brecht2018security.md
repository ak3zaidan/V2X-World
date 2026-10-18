# A Security Credential Management System for V2X Communications
Benedikt Brecht, Dean Therriault, André Weimerskirch, William Whyte, Virendra Kumar, Thorsten Hehn et al. · IEEE Transactions on Intelligent Transportation Systems 2018 · https://doi.org/10.1109/tits.2018.2797529 · relevance: high · basis: abstract

**Problem:** Security and privacy for the US V2V mandate.
**Approach:** CAMP/USDOT SCMS design: bootstrapping, certificate provisioning, misbehaviour reporting and revocation; pseudonym certificates; separation of duties across organisations; butterfly key expansion and linkage values (per the paper).
**Tools and scale:** System design; proof-of-concept transition.
**Results:** Design (qualitative); efficient provisioning and revocation mechanisms.
**Limitations and future work (as stated by the authors):**
- None stated in abstract.
**Overlap with our engine:** strong - this is the SCMS architecture our engine implements (RA, PCA, LA, MA, ECA, CRL, butterfly keys, linkage-based revocation).
**Novelty implications:** The SCMS design itself is not ours to claim; novelty must lie in executable, validated, closed-loop simulation of it at city scale and the studies it enables.
