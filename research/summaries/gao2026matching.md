# Matching Comes First: Efficient Certificateless Lattice-Based Bilateral Access Control With On-Demand Matching
Xin Gao, Hua-Qun Wang, Mengxue Yang, Hua Dai et al. · IEEE Transactions on Information Forensics and Security 2026 · https://doi.org/10.1109/tifs.2026.3703180 · relevance: low · basis: abstract

**Problem:** The proliferation of data-driven services on cloud platforms, coupled with stringent regulatory frameworks like the General Data Protection Regulation (GDPR), necessitates cryptographic solutions that ensure secure and efficient data exchange. However, simultaneously achieving post-quantum security, bilateral access control, data authenticity, simplified key management and ...
**Approach:** To address these issues, this paper introduces a certificateless lattice-based matchmaking encryption (CLLME) to provide post-quantum security while obviating key escrow and certificate management. The proposed scheme enforces bilateral access control, allowing both data senders and receivers to specify matching access structures; decryption is thus contingent upon mutual authorization.
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** We formally prove that CLLME achieves indistinguishability against chosen-plaintext attacks and existential unforgeability against chosen-message attacks under lattice-based assumptions. Experimental evaluations demonstrate that CLLME maintains favorable communicational and computational efficiency, confirming its suitability for practical and scalable deployment in regulation-compliant, large-scale data sharing ...
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
