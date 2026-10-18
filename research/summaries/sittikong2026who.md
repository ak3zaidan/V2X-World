# Who Gets Post-Quantum TLS? Measuring Thai Personal-Data and Popular Hosts
Badeesorn Sittikong, Tuul Triyason · venue not stated 2026 · https://doi.org/10.2139/ssrn.7576653 · relevance: low · basis: abstract

**Problem:** Personal data that must stay confidential for decades, such as tax, health and identity records, is exposed to ”harvest now, decrypt later”: traffic recorded today could be decrypted once quantum computers mature. Hybrid post-quantum key exchange for TLS is enabled by default in major browsers, content delivery networks (CDNs) and OpenSSL, but existing measurements count ...
**Approach:** We measured 288 Thai banks, insurers, hospitals, government agencies and regulators from two networks, including web endpoints coded as handling personal data and service hosts referenced by 20 banking and government apps, and 1,040 popular .th hosts drawn from the Chrome UX Report. Post-quantum key exchange followed the TLS terminator: 97.4% of organisational web hosts behind a CDN negotiated it, against 3.6% of ...
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** Only about 40% of popular hosts carrying personal data were protected, and hosts classified as institutional channels, such as hospital claims systems, appeared least protected (7–17%), though concentrated in a few organisations. Service hosts referenced in apps were protected far less often than the same apps' content hosts (30.9% against 66.1%), while web data endpoints usually matched their organisation's ...
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
