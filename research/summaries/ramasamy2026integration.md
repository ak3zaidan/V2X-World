# Integration of NIST PQC Round 2 Additional Digital Signatures into PKI and X. 509
Amrut Ramasamy, Sanketh Chapaneri, Ryan Slovensky, Ken Lew et al. · 2026 International Conference on Intelligent Multimedia, Networking, and Security (IMNS) 2026 · https://doi.org/10.1109/imns67862.2026.11655251 · relevance: low · basis: abstract

**Problem:** Public Key Infrastructure (PKI) allows the issuance, management, and validation of digital certificates to establish trust between entities in networked systems. The security of PKI relies on cryptography and the underlying hardness problems.
**Approach:** In this work, we present a comprehensive performance analysis of these 14 additional signature candidates, evaluating their integration and operational overhead within a functional X. 509 PKI environment.
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** Eight of the fourteen Round 2 candidates outperform SLH-DSA in signing efficiency, with schemes such as HAWK and CROSS achieving performance levels that approach those of ML-DSA and FALCON.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
