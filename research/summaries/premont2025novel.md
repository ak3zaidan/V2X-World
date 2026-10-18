# A Novel Optimized Encoding Approach for Certificate Revocation
Arthur Premont, Hossam Afifi, Hassine Moungla · ICC 2025 - IEEE International Conference on Communications 2025 · https://doi.org/10.1109/icc52391.2025.11161161 · relevance: low · basis: abstract

**Problem:** One of the goals of a PKI (Public Key Infrastructure), which is ubiquitous in our systems and networks, is to be able to create a trusted association — represented by a certificate — between a public key and an entity; It is however, of the same importance, to be able to revoke this trust.
**Approach:** Two existing and widely used methods are presented: certificate revocation lists (CRLs) and the Online Certificate Status Protocol (OCSP).
**Tools and scale:** Not stated in the abstract.
**Results:** In this article, a new encoding scheme (LightyCoding) is proposed, which is no longer based on the historical ASN.1 standard, and which offers a lightweight structure and improved performance. The results are a significant reduction in CRL size of around 50% for most uses and 30% for OCSP, but still containing the same useful data as an ASN.1-encoded revocation.
**Limitations and future work (as stated by the authors):**
- A major challenge is to efficiently share the revocations that took place.
- Both mechanisms can be optimized but have limitations, especially due to their cumbersome nature.
**Overlap with our engine:** none — low-relevance adjacent work (general PKI/revocation background); no SCMS/1609.2 PKI backend, closed-loop revocation, joint ground truth or counterfactual reruns indicated.
**Novelty implications:** Does not rule out any candidate direction; background only.
