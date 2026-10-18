# Analyzing Attacks on Cooperative Adaptive Cruise Control (CACC)
Rens Wouter van der Heijden, Thomas Lukaseder, Frank Kargl · IEEE Vehicular Networking Conference (VNC) 2017 · https://doi.org/10.1109/vnc.2017.8275598 · relevance: high · basis: full text

**Problem:** CACC relies on exchanged information and needs resilience against attacks.
**Approach:** Rigorous attacker model and evaluation framework quantifying attack impact on three CACC controllers under jamming and data injection.
**Tools and scale:** Platoon simulation (PLEXE/Veins-based per full text context); 802.11p.
**Results:** All three controllers can be attacked effectively and easily via jamming or data injection, producing many crashes; resilience differs between controllers.
**Limitations and future work (as stated by the authors):**
- Detection alone is likely insufficient given the volume of crashes; need prevention via resilient controllers and effective detection and revocation.
- Future work: other controllers; falsifying multiple beacon fields simultaneously (evades typical MBD).
- Metrics bound to the leader's behaviour.
- Open: how to use detection beyond invalidating the vehicle in the SCMS; combine graceful degradation with effective revocation.
**Overlap with our engine:** strong — quantifies attack impact on safety (crashes) in V2X simulation and explicitly calls for coupling detection with SCMS revocation, which is direction 1.
**Novelty implications:** Attack-to-crash quantification for platoons is established; open (stated by them): the combined detection + revocation + graceful degradation loop, which our closed-loop engine targets.
