# The Performance of Post-Quantum TLS 1.3
Markus Sosnowski, Florian Wiedner, Eric Hauser et al. · Companion of the 19th International Conference on emerging Networking EXperiments and Technologies 2023 · https://doi.org/10.1145/3624354.3630585 · relevance: low · basis: abstract

**Problem:** Quantum Computers (QCs) differ radically from traditional computers and can efficiently solve mathematical problems fundamental to our current cryptographic algorithms.
**Approach:** Different signature algorithms and key agreements (as proposed by the National Institute of Standards and Technology (NIST)) are examined through black- and white-box measurements to get precise handshake latencies and computational costs per participating library.
**Tools and scale:** Not stated in the abstract.
**Results:** Our results reveal that HQC and Kyber are on par with our current state-of-the-art, while Dilithium and Falcon are even faster. We observed no performance drawback from using hybrid algorithms; moreover, on higher NIST security levels, PQC outperformed any algorithm in use today.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Security/authentication scheme for vehicular networks; overlaps the 1609.2/SCMS layer in topic but not the US SCMS design (butterfly keys, LA/MA, linkage-based CRL) or simulation-integrated evaluation.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
