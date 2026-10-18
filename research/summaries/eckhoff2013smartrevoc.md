# SmartRevoc: An efficient and privacy preserving revocation system using parked vehicles
David Eckhoff, Falko Dressler, Christoph Sommer · Conference on Local Computer Networks 2013 · https://doi.org/10.1109/lcn.2013.6761338 · relevance: high · basis: abstract

**Problem:** CRL size, dissemination speed and location privacy in PKI-based vehicular networks.
**Approach:** SmartRevoc: two hash chains let vehicles compute CRL entries locally, privacy-preserving (no linking past pseudonyms), epidemic CRL dissemination via parked vehicles.
**Tools and scale:** Veins simulation (OMNeT++/SUMO).
**Results:** Reduced communication overhead; few parked vehicles outperform an unreasonably high number of RSUs for dissemination.
**Limitations and future work (as stated by the authors):**
- None stated in abstract.
**Overlap with our engine:** strong — hash-chain-based pseudonym revocation (similar in spirit to SCMS linkage values) and CRL dissemination speed simulation, overlapping our CRL broadcast and direction 1.
**Novelty implications:** Cannot claim first CRL dissemination-speed simulation or hash-chain CRL compression; open: real SCMS LA linkage CRLs with safety outcome.
