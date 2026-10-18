# Scalable & Resilient Vehicle-Centric Certificate Revocation List Distribution in Vehicular Communication Systems
Mohammad Khodaei, Panos Papadimitratos · IEEE Transactions on Mobile Computing 2020 · https://doi.org/10.1109/tmc.2020.2981887 · relevance: high · basis: full text (extracted sections)

**Problem:** No consensus on CRL distribution for numerous pseudonyms under privacy and OBU constraints; peers can pollute distribution.
**Approach:** Vehicle-centric CRL distribution: vehicles fetch CRL pieces only for their region and trip duration; CRL-piece fingerprints in pseudonyms for fast validation; delta-CRLs pushed via RSUs.
**Tools and scale:** Experimental evaluation on a 15x15 km region (realistic mobility per paper).
**Results:** With at most 25 KB/s traffic, latest CRL reaches 95% of vehicles within 15 s, more than 40x faster than state of the art.
**Limitations and future work (as stated by the authors):**
- None stated in extracted sections.
**Overlap with our engine:** partial (near strong) - quantifies CRL dissemination latency in a VPKI with RSUs and V2V in a large region; our engine does CRL broadcast via RSU relay inside the SCMS loop.
**Novelty implications:** Cannot claim first quantification of CRL dissemination latency at city scale. Open: linkage-seed CRL of the US SCMS and its end-to-end effect on time-to-neutralise and safety.
