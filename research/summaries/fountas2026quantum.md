# A Quantum-Agile Framework for Secure 5G Core Communication Using Post-Quantum TLS
Vasileios Fountas, Grigorios Kakaletsis, Harry Skianis, Emmanouil Kafetzakis et al. · ICC 2026 - IEEE International Conference on Communications 2026 · https://doi.org/10.1109/icc59461.2026.11588003 · relevance: low · basis: abstract

**Problem:** The rise of quantum computing poses a critical threat to current cryptographic protocols, particularly RSA and Elliptic Curve Cryptography (ECC), which are vulnerable to attacks such as Shor’s algorithm. As 5G networks rely heavily on public key infrastructure, the risk of compromise in the quantum era is significant.
**Approach:** This paper presents a practical framework for integrating PQC into 5G control plane communication, specifically over Service-Based Interfaces (SBIs). Using the National Institute of Standards and Technology (NIST) finalist algorithm Dilithium3, we implement PQC-enabled Transport Layer Security (TLS) handshakes via a modular proxy built in Python.
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** The abstract gives no specific quantitative results.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
