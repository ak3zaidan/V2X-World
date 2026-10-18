# Design and Implementation of Certificate Revocation List Acquisition Method for Security of Vehicular Communications
Hyun-Gon Kim · 한국통신학회논문지 2012 · https://doi.org/10.7840/kics.2012.37.7c.584 · relevance: high · basis: abstract

**Problem:** Sparse RSU deployment makes timely CRL distribution hard.
**Approach:** Nomadic-device-based CRL acquisition: a phone acts as security mediator over cellular when no RSU is reachable. (summarised from the abstract only)
**Tools and scale:** Design and implementation; no scale stated.
**Results:** Vehicles receive recent CRLs in reasonable time even with few RSUs.
**Limitations and future work (as stated by the authors):**
- Not stated.
**Overlap with our engine:** partial — CRL delivery over cellular when RSUs are sparse; our engine fetches CRLs over cellular TCP/TLS and via RSU broadcast.
**Novelty implications:** Cellular CRL fetching is old; any claim must be about measured revocation latency vs safety, not the mechanism.
