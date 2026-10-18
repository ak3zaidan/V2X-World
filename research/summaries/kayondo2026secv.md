# SECV: Securing Connected Vehicles with Hardware Trust Anchors
Martin Kayondo, Junseung You, Eunmin Kim et al. · Proceedings 2026 Network and Distributed System Security Symposium 2026 · https://doi.org/10.14722/ndss.2026.240106 · relevance: low · basis: abstract

**Problem:** —Modern vehicles integrate Extra-Vehicle Networks (EVNs) with In-Vehicle Networks (IVNs) to support navigation, diagnostics, and over-the-air updates.
**Approach:** We present S E CV, a runtime security mechanism that enables the IVN gateway to accurately verify EVN-originated control messages even when the EVN manager is compromised.
**Tools and scale:** Not stated in the abstract.
**Results:** 5% transmission geometric mean overhead and 1 . 5% additional message loss during extreme communication bursts, effectively mitigating EVN-originated attacks while satisfying real-time constraints.
**Limitations and future work (as stated by the authors):**
- S E CV addresses practical challenges in TEE–HSM trust establishment, real-time mediation, and robust attribution under compromise.
**Overlap with our engine:** none — low-relevance adjacent work (vehicular security, related: message authentication); no SCMS/1609.2 PKI backend, closed-loop revocation, joint ground truth or counterfactual reruns indicated.
**Novelty implications:** Does not rule out any candidate direction; background only.
