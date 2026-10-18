# Post-Quantum TLS on Embedded Systems: Integrating and Evaluating Kyber and SPHINCS+ with mbed TLS
Kevin Bürstinghaus-Steinbach, Christoph Krauß, Ruben Niederhagen et al. · Proceedings of the 15th ACM Asia Conference on Computer and Communications Security 2020 · https://doi.org/10.1145/3320269.3384725 · relevance: low · basis: abstract

**Problem:** We present our integration of post-quantum cryptography (PQC), more specifically of the post-quantum KEM scheme Kyber for key establishment and the post-quantum signature scheme SPHINCS+, into the embedded TLS library mbed TLS.
**Approach:** We measure the performance of these post-quantum primitives on four different embedded platforms with three different ARM processors and an Xtensa LX6 processor.
**Tools and scale:** Not stated in the abstract.
**Results:** Our results show that post-quantum key establishment with Kyber performs well in TLS on embedded devices compared to ECC variants.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Security/authentication scheme for vehicular networks; overlaps the 1609.2/SCMS layer in topic but not the US SCMS design (butterfly keys, LA/MA, linkage-based CRL) or simulation-integrated evaluation.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
