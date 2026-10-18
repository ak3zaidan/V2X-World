# Investigating Safety Effects of PKI Authentication, in Automotive Systems
Zsombor Pethő, Tamás Márton Kazár, Roland Kraudy, Zsolt Szalay et al. · Acta Polytechnica Hungarica 2024 · https://doi.org/10.12700/aph.21.7.2024.7.13 · relevance: high · basis: full text

**Problem:** Safety effect of PKI authentication overhead.
**Approach:** Quantifies safety risk versus QoS parameters with and without PKI authentication.
**Tools and scale:** Analytical/simulation test cases; industry co-author (Microsec).
**Results:** At low PDR risk exceeds 0.9; PKI raises risk by at most about 0.11; near 100% PDR overhead is negligible; PKI safety impact judged insignificant.
**Limitations and future work (as stated by the authors):**
- Increased processing time is unsafe only if QoS drops below acceptable levels.
**Overlap with our engine:** strong — directly measures the safety cost of security processing (direction 2).
**Novelty implications:** A first quantification exists and finds small effect; a city-scale, closed-loop evaluation with PQ signatures and verification policies remains open, and must engage with this result.
