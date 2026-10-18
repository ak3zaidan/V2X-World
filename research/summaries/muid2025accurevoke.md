# AccuRevoke: Enhancing Certificate Revocation with Distributed Cryptographic Accumulators
Munshi Rejwan Ala Muid, Taejoong Chung, Thang Hoang · 2025 IEEE Symposium on Security and Privacy (SP) 2025 · https://doi.org/10.1109/sp61157.2025.00129 · relevance: low · basis: abstract

**Problem:** Certificate revocation is essential for maintaining the security of the Public Key Infrastructure (PKI), ensuring that compromised or untrustworthy certificates are invalidated promptly.
**Approach:** In this paper, we introduce AccuRevoke, a novel revocation scheme that leverages cryptographic accumulators and edge computing to address these challenges effectively.
**Tools and scale:** Not stated in the abstract.
**Results:** Our scheme significantly reduces bandwidth consumption by providing compact revocation proofs-approximately 21 bytes for membership proofs and 61 bytes for non-membership proofs-which are substantially smaller than traditional OCSP responses. To further optimize performance, especially in generating non-membership witnesses, we employ GPU acceleration, achieving considerable improvements in processing times.
**Limitations and future work (as stated by the authors):**
- Traditional revocation mechanisms like Certificate Revocation Lists (CRLs) and the Online Certificate Status Protocol (OCSP) face significant challenges, including scalability issues, high bandwidth consumption, privacy concerns, and reliance on centralized infrastructure that can become points of f
- In this paper, we introduce AccuRevoke, a novel revocation scheme that leverages cryptographic accumulators and edge computing to address these challenges effectively.
**Overlap with our engine:** none — low-relevance adjacent work (general PKI/revocation background); no SCMS/1609.2 PKI backend, closed-loop revocation, joint ground truth or counterfactual reruns indicated.
**Novelty implications:** Does not rule out any candidate direction; background only.
