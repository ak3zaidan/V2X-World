# Certificate status information distribution and validation in vehicular networks
Carlos Hernández Gañán · thesis 2013 · https://doi.org/10.5821/dissertation-2117-95623 · relevance: high · basis: full text

**Problem:** Revocation and certificate status distribution in VANET PKI is an open problem.
**Approach:** PhD thesis: models the revocation process (finds it statistically self-similar, ARFIMA synthetic trace generator), PKI deployment in VANETs, a certificate status checking mechanism with a risk indicator combining CRL age and revoked count, and a game-theoretic model of revocation service and certificate prices.
**Tools and scale:** Real CA revocation data (e.g. GoDaddy); analytical models; VANET simulation.
**Results:** Revocation is self-similar over four orders of magnitude; providers with better revocation information can charge more without losing share (Bertrand equilibrium).
**Limitations and future work (as stated by the authors):**
- Not clearly stated in extracted sections.
**Overlap with our engine:** partial — revocation-process modelling and probability of accepting a revoked certificate as a function of CRL age, related to direction 1.
**Novelty implications:** Risk of accepting revoked certificates vs CRL age has been modelled; open: closed-loop measurement inside a full SCMS with real traffic.
