# From TS-SUF-2 to TS-SUF-4: Practical Security Enhancements for FROST2 Threshold Signatures
Will Wang, Syh-Yuan Tan, Ryan Chow, Chanson Chan et al. · arXiv 2026 · https://arxiv.org/abs/2610.02805 · relevance: low · basis: abstract

**Problem:** Threshold signature schemes play a vital role in securing digital assets within blockchain and distributed systems. FROST2 stands out as a practical threshold Schnorr signature scheme, noted for its efficiency and compatibility with standard verification processes.
**Approach:** In this paper, we address this security limitation by presenting an enhanced variant of FROST2, namely, FROST2+ which achieves the TS-SUF-4 security level under the same computational assumptions as the original FROST2. We show that FROST2+ can achieve TS-SUF-4 security not only under the same conditions as the original FROST2 analysis, but also when initialized with a distributed key generation protocol such as ...
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** Our benchmark using ZCash's FROST library shows that the performance of FROST2+ is comparable to FROST2 and about 64-79% faster than FROST when precomputation is enabled.
**Limitations and future work (as stated by the authors):**
- "In this paper, we address this security limitation by presenting an enhanced variant of FROST2, namely, FROST2+ which achieves the TS-SUF-4 security level under the same computational assumptions as the original FROST2."
**Overlap with our engine:** none: threshold/multi-party signatures (general, non-vehicular). Our threshold authorities (FROST, CGGMP21) build on this family of schemes; nothing here applies them to an SCMS.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
