# Experimental Evaluation of Post-Quantum PKI Migration: Mixed Certificate Chains, Trust Anchors, and CA Strategies
Hridoy Roy · venue not stated 2026 · https://doi.org/10.2139/ssrn.7444038 · relevance: low · basis: abstract

**Problem:** Post-quantum migration of Public Key Infrastructure (PKI) is not equivalent to replacing a classical signature algorithm with a post-quantum alternative. Certificates, certification authorities (CAs), trust anchors, validation software, and relying-party trust stores form a dependency graph in which cryptographic changes propagate through multiple system layers.
**Approach:** This paper extends a prior systematization of post-quantum PKI migration with a controlled experimental study of X.509 certificate chains.
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** The measured results show substantial certificate-chain expansion for standardized post-quantum signatures relative to the ECDSA baseline: the transmitted two-certificate chain grows from 1,090 bytes for ECDSA P-256 to 8,314 bytes for ML-DSA-44, 11,373 bytes for ML-DSA-65, 15,288 bytes for ML-DSA-87, 16,685 bytes for SLH-DSA-SHA2-128s, and 35,150 bytes for SLH-DSA-SHA2-128f. Validation latency increases more ...
**Limitations and future work (as stated by the authors):**
- "The paper concludes with evidence-based migration recommendations, a controlled configuration framework, and explicit limitations concerning implementation scope, hardware, interoperability, and production-scale deployment."
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
