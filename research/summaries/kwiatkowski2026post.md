# Post-Quantum Traditional (PQ/T) Hybrid Key Agreement Mechanisms for TLS 1.3
K. Kwiatkowski, P. Kampanakis, B. E. Westerbaan, D. Stebila · venue unknown 2026 · https://doi.org/10.17487/rfc10024 · relevance: low · basis: abstract

**Problem:** This document defines three hybrid key agreement mechanisms for TLS 1.3 -- X25519MLKEM768, SecP256r1MLKEM768, and SecP384r1MLKEM1024 -- that combine the post-quantum ML-KEM (Module-Lattice-Based Key Encapsulation Mechanism) with an ECDHE (Ephemeral Elliptic Curve Diffie-Hellman) exchange.
**Approach:** This document defines three hybrid key agreement mechanisms for TLS 1.3 -- X25519MLKEM768, SecP256r1MLKEM768, and SecP384r1MLKEM1024 -- that combine the post-quantum ML-KEM (Module-Lattice-Based Key Encapsulation Mechanism) with an ECDHE (Ephemeral Elliptic Curve Diffie-Hellman) exchange.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** No quantitative results stated in the abstract.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — post-quantum/hybrid authentication scheme; the engine implements PQ and hybrid signatures, but the abstract states no SCMS-scale or traffic-coupled evaluation.
**Novelty implications:** We cannot claim PQ authentication for vehicles is new; the end-to-end safety cost of PQ in a full SCMS remains open as far as this abstract shows.
