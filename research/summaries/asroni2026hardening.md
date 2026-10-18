# Hardening Pairing-Free Certificate-Based Aggregate Signatures for VANETs Against Transcript Leakage
Asroni, S. Sulistyo, S. B. Wibowo · 2026 International Conference on Information Technology and Computing (ICITCOM) 2026 · https://doi.org/10.1109/icitcom70739.2026.11714366 · relevance: medium · basis: abstract

**Problem:** Pairing-free certificate-based aggregate signature (CBAS) schemes are attractive for VANETs because they avoid bilinear pairings while supporting batch authentication.
**Approach:** This paper gives a transcript-aware hardening analysis of a recurring linear signer-side structure in representative pairing-free CBAS schemes; it does not claim a new generic nonce-recovery attack. We show that repeated same-context signing exposes an algebraic transcript relation that is harmless with full-size random nonces but exploitable under scoped implementation failures such as a reduced nonce space or an ...
**Tools and scale:** We show that repeated same-context signing exposes an algebraic transcript relation that is harmless with full-size random nonces but exploitable under scoped implementation failures such as a reduced nonce space or an observable certificate scalar.
**Results:** MIRACL/AMCL measurements put CBAS-DSH's per-signer verification at about 0.92 ms, roughly $6.87 \times$ cheaper than one bilinear pairing (1.261 ms), and a companion ProVerif model confirms authentication correspondence and secrecy for both schemes.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — cryptographic/authentication scheme or analysis without the closed-loop simulation, SCMS backend or counterfactual machinery of our engine.
**Novelty implications:** No effect on our novelty claims. 
