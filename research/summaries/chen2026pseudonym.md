# Pseudonym Scheme Based on Hybrid Certificates for Security Credential Management System in Vehicular Communications
Abel C. H. Chen, F. J. Hwang, Yu-Chih Wei, Chin-Chen Chang et al. · IEEE Canadian Journal of Electrical and Computer Engineering (2026) 2026 · https://doi.org/10.1109/icjece.2026.3692881 · relevance: high · basis: full text

**Problem:** SCMS and butterfly key expansion rely on ECC, vulnerable to quantum attacks.
**Approach:** Hybrid certificates (CA uses Falcon-512, end entities ECDSA P-256), a generalised algorithm-agnostic pseudonym scheme preventing correlation between enrolment and pseudonym public keys, KEM-based encrypted SPDU per IEEE 1609.2, and benchmarks of RSA, ECC, PQC.
**Tools and scale:** Implementation on resource-constrained end entity; message length and timing analysis.
**Results:** Falcon-512 CA + ECDSA EE keeps PDU under the 1400-byte V2V threshold; Falcon-512 verification faster than ECDSA P-256 on the tested EE; unlinkability between enrolment and pseudonym keys argued.
**Limitations and future work (as stated by the authors):**
- Not explicitly stated in extracted text.
**Overlap with our engine:** strong — PQ/hybrid certificates inside the SCMS, a PQ-compatible replacement for butterfly key expansion and KEM-encrypted 1609.2 SPDUs, which our PQ/hybrid SCMS implements.
**Novelty implications:** We cannot claim the first hybrid-certificate SCMS pseudonym scheme or PQ replacement of butterfly keys; open: system-level evaluation of such schemes in a running SCMS with traffic and radio.
