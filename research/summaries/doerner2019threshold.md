# Threshold ECDSA from ECDSA Assumptions: The Multiparty Case
Jack Doerner, Yashvanth Kondi, Eysa Lee et al. · 2019 IEEE Symposium on Security and Privacy (SP) 2019 · https://doi.org/10.1109/sp.2019.00024 · relevance: low · basis: abstract

**Problem:** Cryptocurrency applications have spurred a resurgence of interest in the computation of ECDSA signatures using threshold protocols---that is, protocols in which the signing key is secret-shared among n parties, of which any subset of size t must interact in order to compute a signature.
**Approach:** We propose an extension of their scheme to arbitrary thresholds, and prove it secure against a malicious adversary corrupting up to one party less than the threshold under only the Computational Diffie-Hellman assumption in the Random Oracle model, an assumption strictly weaker than those under which ECDSA is proven. We design a new consistency check, similar in spirit to that of Doerner et al., but suitable for an arbitrary number of participants, and we optimize the underlying two-party multiplication protocol on which our scheme is based, reducing its concrete communication and computation costs.
**Tools and scale:** Not stated in the abstract.
**Results:** Among the resulting works to date, that of Doerner et al. requires the most natural assumptions while also achieving the best practical signing speed. We find that in the LAN setting, our scheme outperforms all prior works by orders of magnitude, and that it is efficient enough for use even on smartphones or hardware tokens.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Security/authentication scheme for vehicular networks; overlaps the 1609.2/SCMS layer in topic but not the US SCMS design (butterfly keys, LA/MA, linkage-based CRL) or simulation-integrated evaluation.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
