# Trust-Aware Sybil Attack Detection for Resilient Vehicular Communication
Mortan Thomas, Abinash Borah, Anirudh Paranjothi · Internet Technology Letters, 2024 2024 · https://doi.org/10.1002/itl2.617 · relevance: high · basis: full text

**Problem:** Sybil detection suffers delays and infrastructure reliance.
**Approach:** TASER: cluster trust metric plus directional-antenna challenge of GPS positions.
**Tools and scale:** OMNeT++, SUMO, Veins.
**Results:** Up to 66% shorter detection time in urban scenarios; accuracy varies at most 3% up to 30% Sybil nodes.
**Limitations and future work (as stated by the authors):**
- Integrate more vehicle-level data
- share trust observations between vehicles
- integrate with identity management for reporting malicious actors
**Overlap with our engine:** partial — V2V Sybil detection; explicitly leaves MBD-to-reporting/revocation integration open.
**Novelty implications:** Supports open status of detection-to-revocation closed loop.
