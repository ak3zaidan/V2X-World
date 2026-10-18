# Measuring Post-Quantum TLS Deployment Across UK Internet Sectors
Konstantinos Loizou, Essam Ghadafi · venue not stated 2026 · https://arxiv.org/abs/2608.02147 · relevance: low · basis: abstract

**Problem:** Post-quantum cryptography (PQC) is becoming an important component of long-term trust in Internet-facing infrastructure. Publicly observable PQC support provides evidence of externally visible deployment, but does not necessarily reflect the overall progress of an organisation's post-quantum migration.
**Approach:** We present a measurement study of observable PQC deployment across 4,665 UK organisations spanning ten sectors. We measure post-quantum key-exchange support across HTTPS and SMTP STARTTLS endpoints, attribute reachable endpoints to their underlying infrastructure providers where possible, and statistically examine protocol-, sector-, and provider-level deployment patterns.
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** Among reachable endpoints, 44.0\% of HTTPS services supported at least one evaluated PQC key-exchange group, compared with 6.4\% of SMTP services.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
