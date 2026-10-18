# secp256k1-frost: Minimal-dependency RFC-9591 FROST threshold signing for secp256k1
Matteo Nardelli, Antonio Muci · SoftwareX 2026 · https://doi.org/10.2139/ssrn.6349435 · relevance: low · basis: abstract

**Problem:** Flexible Round-Optimized Schnorr Threshold Signatures (FROST), standardized in IETF RFC-9591, enable distributed generation of compact Schnorr signatures while preserving compatibility with existing secp256k1 verification workflows. This paper presents secp256k1-frost, a minimal-dependency implementation developed as an extension module for the secp256k1 library.
**Approach:** This paper presents secp256k1-frost, a minimal-dependency implementation developed as an extension module for the secp256k1 library.
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** The abstract gives no specific quantitative results.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: threshold/multi-party signatures (general, non-vehicular). Our threshold authorities (FROST, CGGMP21) build on this family of schemes; nothing here applies them to an SCMS.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
