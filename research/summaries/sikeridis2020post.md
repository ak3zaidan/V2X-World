# Post-Quantum Authentication in TLS 1.3: A Performance Study
Dimitrios Sikeridis, Panos Kampanakis, Michael Devetsikiotis · Proceedings 2020 Network and Distributed System Security Symposium 2020 · https://doi.org/10.14722/ndss.2020.24203 · relevance: low · basis: abstract

**Problem:** —The potential development of large-scale quantum computers is raising concerns among IT and security research professionals due to their ability to solve (elliptic curve) discrete logarithm and integer factorization problems in polynomial time.
**Approach:** Finally, we propose and evaluate the combination of different PQ signature algorithms across the same certiﬁcate chain in TLS. Results show a reduction of the TLS handshake time and a signiﬁcant increase of a server’s TLS tunnel connection rate over using a single PQ signature scheme.
**Tools and scale:** Not stated in the abstract.
**Results:** Our results demonstrate that the adoption of at least two PQ signature algorithms would be viable with little additional overhead over current signature algorithms. Also, we argue that many NIST PQ candidates can effectively be used for less time- sensitive applications, and provide an in-depth discussion on the integration of PQ authentication in encrypted tunneling protocols, along with the related challenges, improvements, and alternatives.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Security/authentication scheme for vehicular networks; overlaps the 1609.2/SCMS layer in topic but not the US SCMS design (butterfly keys, LA/MA, linkage-based CRL) or simulation-integrated evaluation.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
