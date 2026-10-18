# Comparison of Credential Management Systems Based on the Standards of IEEE, ETSI, and YD/T 3957-2021
Abel C. H. Chen · arXiv.org 2026 · https://arxiv.org/abs/2603.03376 · relevance: high · basis: full text

**Problem:** IEEE 1609.2/SCMS, ETSI CCMS and Chinese YD/T 3957-2021 credential systems differ, hindering compatibility.
**Approach:** Comparison of certificate formats, signed message formats and certificate request processes; benchmarks of P-256 vs SM2, SHA-256 vs SM3, AES-128 vs SM4; implementation of all three on V2X devices.
**Tools and scale:** Real V2X devices; their RCA certificate passed OmniAir Plugfest interoperability testing and is listed in the SCMS Manager test CTL and the EU ECTL.
**Results:** ECDSA and SM2 key generation and verification similar; SM2 signing slower due to extra hashing; device-level sign/verify efficiency compared.
**Limitations and future work (as stated by the authors):**
- Future work: implement implicit certificates in CCMS and C-SCMS to compare all three standards with implicit certificates.
**Overlap with our engine:** strong — implements the US SCMS credential formats and request processes on devices and interoperates with the real SCMS Manager trust lists, as our engine does.
**Novelty implications:** We cannot claim to be the only academic implementation listed against real SCMS Manager trust lists; open: cross-standard interoperability inside a full simulated multi-provider SCMS.
