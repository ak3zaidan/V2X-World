# Post-Quantum TLS Without Handshake Signatures
Peter Schwabe, Douglas Stebila, Thom Wiggers · Proceedings of the 2020 ACM SIGSAC Conference on Computer and Communications Security 2020 · https://doi.org/10.1145/3372297.3423350 · relevance: low · basis: abstract

**Problem:** We present KEMTLS, an alternative to the TLS 1.3 handshake that uses key-encapsulation mechanisms (KEMs) instead of signatures for server authentication.
**Approach:** Among existing post-quantum candidates, signature schemes generally have larger public key/signature sizes compared to the public key/ciphertext sizes of KEMs: by using an IND-CCA-secure KEM for server authentication in post-quantum TLS, we obtain multiple benefits.
**Tools and scale:** Not stated in the abstract.
**Results:** In a speed-optimized instantiation, KEMTLS reduces the amount of server CPU cycles by almost 90% compared to TLS 1.3, while at the same time reducing communication size, reducing the time until the client can start sending encrypted application data, and eliminating code for signatures from the server's trusted code base.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Security/authentication scheme for vehicular networks; overlaps the 1609.2/SCMS layer in topic but not the US SCMS design (butterfly keys, LA/MA, linkage-based CRL) or simulation-integrated evaluation.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
