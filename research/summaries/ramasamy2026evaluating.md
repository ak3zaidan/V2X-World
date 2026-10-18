# Evaluating X.509 Certificate Performance of NIST PQC Round 2 Additional Digital Signatures
Amrut Ramasamy, Sanketh Chapaneri, Manohar Raavi · Silicon Valley Cybersecurity Conference 2026 · https://doi.org/10.1109/svcc69905.2026.11642264 · relevance: low · basis: abstract

**Problem:** Recent advancements in quantum computing threaten the cryptographic foundations of Public Key Infrastructure (PKI). Although the National Institute of Standards and Technology (NIST) has standardized three post-quantum digital signature algorithms, namely ML-DSA, SLH-DSA, and FALCON, fourteen additional candidates are currently under evaluation, to diversify the collection of ...
**Approach:** This work presents a comprehensive performance analysis of the fourteen ‘Round 2’ digital signature candidates within a functional X. 509 PKI environment. Unlike prior studies focused on isolated benchmarking, we evaluate the complete PKI life cycle, key generation, CSR creation, as well as certificate signing and verification.
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** While HAWK and CROSS maintain high efficiency, multivariate schemes like UOV and FAEST incur significantly higher memory overheads.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
