# HIRZ: Hybrid Post-Quantum Cryptography for Secure Long-Term Data Archiving
Waraif Alajmi, Fatimah Alsafi, Rama Aldhafyan, Lujain F Alshammari et al. · 2026 23rd International Learning and Technology Conference (L&T) 2026 · https://doi.org/10.1109/lt68265.2026.11592519 · relevance: low · basis: abstract

**Problem:** Organizations managing long-term archives face harvest-now-decrypt-later attacks, where adversaries collect encrypted data today for future quantum decryption. While NIST has standardized post-quantum algorithms (CRYSTALS-Kyber, CRYSTALS-Dilithium), pure post-quantum deployment faces regulatory and infrastructure barriers.
**Approach:** This paper presents Hirz, a hybrid cryptographic system combining ECDH-secp384r1 with Kyber-1024 and Dilithium5 for archival workloads requiring 20–30 year confidentiality.
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** Our implementation using OpenSSL 3.2.0 and liboqs 0.10.1 achieves 18% overhead compared to classical systems (29.7 ms versus 25.2 ms for 100 MB files) on commodity Intel hardware. Comparative evaluation demonstrates that hybrid construction adds only 11% overhead versus pure post-quantum while providing regulatory compliance, backward compatibility, and defense against algorithm vulnerabilities.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
