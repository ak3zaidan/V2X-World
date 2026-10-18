# LCP-CAS: Lattice-Based Conditional Privacy-Preserving Certificateless Aggregation Signature Scheme for Industrial IoT
Lin Shi, Ziyi Chen, Zi-Yan Zhang, Panpan Chen et al. · Entropy 2026 · https://doi.org/10.3390/e28030258 · relevance: low · basis: abstract

**Problem:** Aiming at the challenge that traditional signature schemes struggle to simultaneously achieve efficiency, resistance to quantum attacks, and privacy protection, this paper proposes a lattice-based conditional privacy-preserving certificateless aggregate signature method (LCP-CAS). The scheme adopts an unordered aggregation algorithm to compress multiple signatures, in ...
**Approach:** Its security is based on the hardness of the Ring Short Integer Solution (RSIS) problem, ensuring post-quantum resistance.
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** Security analysis shows that the scheme meets the security requirements, including integrity, non-repudiation, conditional privacy preservation, and resistance to collusion attacks. Compared with existing related schemes, LCP-CAS achieves reduces aggregation and verification overhead while maintaining practicality in large-scale settings such as industrial IoT and device monitoring.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
