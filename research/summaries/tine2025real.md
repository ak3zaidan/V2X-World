# Real-World Evaluation of Protocol-Compliant Denial-of-Service Attacks on C-V2X-based Forward Collision Warning Systems
Jean Michel Tine, Mohammed Aldeen, Abyad Enan, M Sabbir Salek et al. · arXiv 2025 · https://arxiv.org/abs/2508.02805 · relevance: high · basis: full text

**Problem:** Protocol-compliant DoS on C-V2X safety apps.
**Approach:** Real testbed attacks: UDP flooding and oversized BSM flooding over PC5 that comply with 3GPP, SAE J2735 and IEEE 1609.
**Tools and scale:** Commercial Release-14 OBUs, 10 Hz BSMs, FCW based on time-to-collision; real vehicles.
**Results:** UDP flooding reduces PDR by up to 87% and raises latency above 400 ms; FCW performance severely degraded.
**Limitations and future work (as stated by the authors):**
- None stated in the full text.
**Overlap with our engine:** strong — measures safety-application (FCW) degradation from a security-compliant DoS, i.e. attack to safety outcome with real hardware; overlaps direction 2 (verification/receiver load).
**Novelty implications:** We cannot claim first quantification of receiver-overload attacks on FCW; city-scale simulation with verification queues and revocation remains open.
