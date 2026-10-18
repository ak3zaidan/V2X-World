# Post-Quantum Key Exchange for the TLS Protocol from the Ring Learning with Errors Problem
Joppe W. Bos, Craig Costello, Michael Naehrig et al. · IEEE Symposium on Security and Privacy 2015 · https://doi.org/10.1109/sp.2015.40 · relevance: low · basis: abstract

**Problem:** Lattice-based cryptographic primitives are believed to offer resilience against attacks by quantum computers.
**Approach:** Our approach ties lattice-based key exchange together with traditional authentication using RSA or elliptic curve digital signatures: the post-quantum key exchange provides forward secrecy against future quantum attackers, while authentication can be provided using RSA keys that are issued by today's commercial certificate authorities, smoothing the path to adoption.
**Tools and scale:** Not stated in the abstract.
**Results:** Compared to elliptic curve Diffie-Hellman, this means an 8 KiB increased handshake size and a reduction in throughput of only 21%.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Studies V2X attacks or their detection; overlaps the misbehaviour/attack modules but without linkage-based revocation, closed loop to the SCMS, or joint ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
