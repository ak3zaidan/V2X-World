# Verifying Certificate Revocation Status for Short Key Lengths in Vehicle Communication Systems
Eun-Gi Kim · Journal of Machine and Computing 2024 · https://doi.org/10.53759/7669/jmc202404046 · relevance: high · basis: full text (abstract, conclusion/future-work sections)

**Problem:** This paper proposes and analyzes a ticket-based OCSP protocol for efficient certificate revocation checking in vehicle communication systems.
**Approach:** The IEEE WAVE standard for vehicular networks requires real-time processing of Basic Safety Messages (BSMs) exchanged between vehicles. Traditional OCSP revocation checking can introduce delays.
**Tools and scale:** Not stated in the available text unless given above (topic: CRL/revocation).
**Results:** The results show the proposed technique provides faster revocation checking.
**Limitations and future work (as stated by the authors):**
- "In the future, we plan to use the results of this study to conduct additional research to increase the security, safety, and efficiency of automobile communication systems. Data Availability No data was used to support this study."
**Overlap with our engine:** strong — ticket-based OCSP revocation checking under BSM timing; overlaps verification-policy cost in direction 2
**Novelty implications:** Prior work on this exact mechanism; we cannot claim it as new. Open: integration with full radio/traffic stack, per-message ground truth, and safety outcomes in closed loop.
