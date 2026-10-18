# Improved Security Performance for VANET Simulations
Raphael Riebl, Markus Monz, Simon Varga, Leandros Maglaras, Helge Janicke, Ali H. Al-Bayatti et al. · IFAC-PapersOnLine 2016 · https://doi.org/10.1016/j.ifacol.2016.11.173 · relevance: high · basis: abstract

**Problem:** Simulations of European VANETs must include signing/verification overhead, but cryptography slows simulations.
**Approach:** Standard-compliant model for secured ITS-G5 messages (in Vanetza/Artery) that minimises simulation performance penalty of cryptographic computations; usable for HIL.
**Tools and scale:** Artery/Vanetza, ITS-G5, ETSI TS 103 097 security.
**Results:** Reduced simulation-environment penalty of crypto (no numbers in abstract).
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** strong — simulates standard-compliant signed V2X messages and their overhead inside a VANET simulator, as our 1609.2 layer does.
**Novelty implications:** We cannot claim to be first to include standard-compliant message security in a V2X simulator; open: full PKI backend (SCMS) and revocation in the loop.
