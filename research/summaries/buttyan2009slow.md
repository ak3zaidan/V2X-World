# SLOW: A Practical pseudonym changing scheme for location privacy in VANETs
Levente Buttyan, Tamas Holczer, Andre Weimerskirch, William Whyte · 2009 IEEE Vehicular Networking Conference (VNC) 2009 · https://doi.org/10.1109/vnc.2009.5416380 · relevance: high · basis: abstract

**Problem:** Heartbeat messages make vehicles trackable; mix zones need coordination.
**Approach:** SLOW: stop beaconing below a speed threshold (e.g. 30 km/h) and change pseudonym during each silent period.
**Tools and scale:** VNC 2009; simulation.
**Results:** Vehicles at lights or in jams change together without explicit cooperation (quantitative results in paper).
**Limitations and future work (as stated by the authors):**
- Silent periods trade off against safety beaconing at low speed.
**Overlap with our engine:** partial — pseudonym change policy; our engine can model silent periods.
**Novelty implications:** Speed-triggered silence is known; its safety cost under cross-layer identifiers remains open.
