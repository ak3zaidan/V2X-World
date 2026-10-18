# Formal Analysis of V2X Revocation Protocols
Jorden Whitefield, Liqun Chen, Frank Kargl, Andrew Paverd, Steve A. Schneider, Helen Treharne et al. · arXiv (Cornell University) 2017 · https://arxiv.org/abs/1704.07216 · relevance: high · basis: full text

**Problem:** V2X revocation must be unlinkable even as vehicles change pseudonyms; prior analyses were informal.
**Approach:** Tamarin formal models of Rewire (Plain and R-token); finds a known functional flaw (pseudonym change evades revocation) and a new authentication flaw (RA cannot verify revocation confirmation, which can be spoofed); proposes O-token with an extra key pair, co-designed with a formal model.
**Tools and scale:** Tamarin prover; trusted-hardware-based revocation with CA, pseudonym provider and RA; no simulation.
**Results:** Proves functional correctness and authentication properties for O-token.
**Limitations and future work (as stated by the authors):**
- Preservation of privacy properties of revocation protocols left for future analysis
- implementation relies on heartbeats against non-delivery.
**Overlap with our engine:** partial — formal analysis of revocation (Rewire, not SCMS linkage-based CRL); we simulate revocation timing, not proofs.
**Novelty implications:** Formal verification of revocation exists for Rewire; formal or empirical analysis of SCMS LA/MA revocation timing remains.
