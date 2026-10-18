# CertRV: an Efficient Certificate Revocation Scheme via Consortium Blockchain, Chameleon Hash and Cuckoo Filter
Huiying Zhang, Yuxian Chen, Wei Ren et al. · 2024 IEEE International Conference on Web Services (ICWS) 2024 · https://doi.org/10.1109/icws62655.2024.00162 · relevance: low · basis: abstract

**Problem:** Certificate revocation management is one of the primary tasks of Public Key Infrastructure (PKI), and it is crucial to the security of PKI.
**Approach:** To address these issues, this paper proposes a certificate revocation scheme called CertRV based on blockchain, cuckoo filter, and chameleon hash.
**Tools and scale:** Not stated in the abstract.
**Results:** Specifically, to improve certificate query efficiency, we construct a "CertRevokedCF" filter to store revoked certificates, and apply the chameleon hash algorithm on the blockchain to modify certificate status information, significantly reducing storage costs while still providing accurate certificate status information even when the filter produces false positive results. Theoretical analysis and experimental results demonstrate that our scheme outperforms other alternatives in terms of security and performance.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — low-relevance adjacent work (general PKI/revocation background); no SCMS/1609.2 PKI backend, closed-loop revocation, joint ground truth or counterfactual reruns indicated.
**Novelty implications:** Does not rule out any candidate direction; background only.
