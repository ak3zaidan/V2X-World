# A Post-Quantum Aware SCMS for V2X Communications Using Selective Cryptographic Methodology
Ahmed Fikry, Ebrahim Ramadan Diab, Mazen Mohamed, Mostafa Mohamed Atef et al. · International Conference on Advancements in Smart, Secure and Intelligent Computing (ASSIC) 2026 · https://doi.org/10.1109/assic70037.2026.11717405 · relevance: high · basis: abstract

**Problem:** SCMS uses ECDSA, broken by Shor; long-lived enrolment certificates (up to 15 years) face harvest-now threats.
**Approach:** PQA-SCMS: Dilithium (FIPS 204) applied selectively to enrolment certificates, CA certificates and safety-critical DENMs, ECDSA kept for short-lived pseudonyms and CAMs; Kyber for session keys. Containerised prototype with full CA hierarchy, OBU/RSU simulators and security test harness. (summarised from the abstract only)
**Tools and scale:** AMD Ryzen 7, N=1000; DSRC capacity analysis at 50 vehicles.
**Results:** CAM signing 0.06 ms, DENM signing 0.18 ms; selective scheme uses 4.3% of DSRC capacity at 50 vehicles versus 161% for a full PQ CAM path; 100% detection of replay and forgery attacks.
**Limitations and future work (as stated by the authors):**
- Not stated in abstract.
**Overlap with our engine:** strong — PQ migration of an SCMS with a full CA hierarchy and channel-capacity cost, which is our PQ SCMS and direction 2.
**Novelty implications:** We cannot claim the first PQ-aware SCMS or the first selective (per-certificate-type) PQ migration analysis; open: channel-capacity analysis is analytical at 50 vehicles; full-stack radio, city-scale traffic and safety outcomes remain open.
