# Low-Bandwidth Threshold ECDSA via Pseudorandom Correlation Generators
Damiano Abram, Ariel Nof, Claudio Orlandi et al. · 2022 IEEE Symposium on Security and Privacy (SP) 2022 · https://doi.org/10.1109/sp46214.2022.9833559 · relevance: low · basis: abstract

**Problem:** Digital signature schemes are a fundamental component of secure distributed systems, and the theft of a signing-key might have huge real-world repercussions e.g., in applications such as cryptocurrencies.
**Approach:** In this paper, we provide a novel threshold protocol for ECDSA, arguably the most relevant signature scheme in practice.
**Tools and scale:** Not stated in the abstract.
**Results:** Our protocol is the first one where the communication complexity of the preprocessing phase is only logarithmic in the number of ECDSA signatures to be produced later, and it achieves therefore a so-called silent preprocessing. Our protocol achieves active security against any number of arbitrarily corrupted parties.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Security/authentication scheme for vehicular networks; overlaps the 1609.2/SCMS layer in topic but not the US SCMS design (butterfly keys, LA/MA, linkage-based CRL) or simulation-integrated evaluation.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
