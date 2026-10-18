# Hardware Limitations to Secure C-ITS: Experimental Evaluation and Solutions
Francesco Pollicino, Dario Stabili, Luca Ferretti, Mirco Marchetti · IEEE Transactions on Vehicular Technology 2021 · https://doi.org/10.1109/tvt.2021.3122333 · relevance: high · basis: full text

**Problem:** Can automotive hardware meet 1609.2 latency with ECQV and ECDSA?
**Approach:** Open ECQV implementation benchmarked; real road and traffic scenarios; heuristics to reduce signatures to verify.
**Tools and scale:** Automotive-grade boards; real traffic scenarios.
**Results:** Meeting C-ITS latency requirements needs resources many automotive platforms lack.
**Limitations and future work (as stated by the authors):**
- None stated.
**Overlap with our engine:** strong — Verification load versus latency under real traffic and selective verification heuristics, directly overlapping verification-policy part of direction 2.
**Novelty implications:** Cannot claim first showing hardware cannot verify all messages or first selective-verification heuristics; open: safety consequences in closed loop, PQ.
