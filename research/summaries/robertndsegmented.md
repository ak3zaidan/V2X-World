# Segmented viral distribution of certificate revocation lists for the security credential management system
Iii Mushrall Robert · unknown venue n.d. · https://doi.org/10.62791/19991 · relevance: high · basis: abstract

**Problem:** Connected Vehicles technology facilitates communication among vehicles to in-crease road safety by broadcasting Basic Safety Messages (BSMs).
**Approach:** While communication increases traffic awareness to avoid collisions, it also creates a new attack surface, requiring a mechanism to secure Vehicle-to-Vehicle (V2V) communication.Vehicular Public Key Infrastructure (V-PKI) manages security credentials for vehicles, ensuring BSM integrity and maintaining V2V availability while protecting car users' privacy. The leading V-PKI implementation is the Security Credential Management System (SCMS), which allows vehicles to authenticate each other using c...
**Tools and scale:** Not stated in the available text unless given above (topic: CRL/revocation).
**Results:** Novel cooperative mechanisms reduce overhead in V2V communication and increase speed of CRL dissemination while maintaining integrity of the entire list. Simulations,using VEINS, SUMO, and OMNeT++ with models coded in C, validate that this segmented viral method distributes CRL quickly and preserves availability of V2V communication while maintaining CRL integrity.
**Limitations and future work (as stated by the authors):**
- However, the CRL distribution method has not been defined yet.This research focuses on efficient CRL distribution.
- Future work includes experiments in V2V test sites beforeintroducing to the standard.
**Overlap with our engine:** strong — segmented viral CRL distribution for the SCMS; overlaps our CRL broadcast/RSU relay
**Novelty implications:** Prior work on this exact mechanism; we cannot claim it as new. Open: integration with full radio/traffic stack, per-message ground truth, and safety outcomes in closed loop.
