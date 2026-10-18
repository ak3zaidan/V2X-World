# UC Non-Interactive, Proactive, Threshold ECDSA with Identifiable Aborts
Ran Canetti, Rosario Gennaro, Steven Goldfeder et al. · Proceedings of the 2020 ACM SIGSAC Conference on Computer and Communications Security 2020 · https://doi.org/10.1145/3372297.3423367 · relevance: low · basis: abstract

**Problem:** Building on the Gennaro & Goldfeder and Lindell & Nof protocols (CCS '18), we present two threshold ECDSA protocols, for any number of signatories and any threshold, that improve as follows over the state of the art: -- For both protocols, only the last round requires knowledge of the message, and the other rounds can take place in a preprocessing stage, lending to a non-interactive threshold ECDSA protocol. -- Both protocols withstand adaptive corruption of signatories.
**Approach:** Furthermore, they include a periodic refresh mechanism and offer full proactive security. -- Both protocols realize an ideal threshold signature functionality within the UC framework, in the global random oracle model, assuming Strong RSA, DDH, semantic security of the Paillier encryption, and a somewhat enhanced variant of existential unforgeability of ECDSA. -- Both protocols achieve accountability by identifying corrupted parties in case of failure to generate a valid signature.
**Tools and scale:** Not stated in the abstract.
**Results:** No quantitative results stated in the abstract.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Security/authentication scheme for vehicular networks; overlaps the 1609.2/SCMS layer in topic but not the US SCMS design (butterfly keys, LA/MA, linkage-based CRL) or simulation-integrated evaluation.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
