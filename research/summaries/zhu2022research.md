# Research on Secure Communication on In-Vehicle Ethernet Based on Post-Quantum Algorithm NTRUEncrypt
Yuan Zhu, Yi-Peng Liu, Mingzhi Wu, Jin-Zhao Li et al. · Electronics 2022 · https://doi.org/10.3390/electronics11060856 · relevance: high · basis: full text

**Problem:** In-vehicle Ethernet session key negotiation needs PQ security.
**Approach:** NTRUEncrypt-enhanced session key negotiation within TLS on in-vehicle Ethernet, compared to ECDH and RSA.
**Tools and scale:** Infineon TC397 microcontroller measurements.
**Results:** At 128-bit security NTRU negotiation is 66.06x faster than ECDH and 1530.98x faster than RSA; at 256-bit 108.52x and 14,845.24x; ROM/RAM use under 0.30%/0.29%.
**Limitations and future work (as stated by the authors):**
- None stated beyond expectation of TLS adoption
**Overlap with our engine:** none — In-vehicle network crypto, not V2X.
**Novelty implications:** None.
