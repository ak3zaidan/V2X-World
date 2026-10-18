# Implementation of MEC-Assisted Collective Perception in an Integrated Artery/Simu5G Simulation Framework
Gergely Attila Kovács, László Bokor · Sensors 2023 · https://doi.org/10.3390/s23187968 · relevance: high · basis: full text

**Problem:** Testing MEC-assisted Collective Perception requires integrated V2X + 5G simulation.
**Approach:** Integrates Simu5G with Artery and extends Simu5G edge to dispatch CPMs to a MEC server.
**Tools and scale:** OMNeT++, Artery, Simu5G; MEC apps with 300 MIPS CPU requirement; max 25 objects per CPM.
**Results:** Operability shown; response times rise with CPU requirement.
**Limitations and future work (as stated by the authors):**
- Parameters chosen as proof of concept, not real implementation
- CPM object count capped at 25
- refine dynamic processing-time model with real data.
**Overlap with our engine:** partial — Artery + 5G co-simulation; no PKI.
**Novelty implications:** No impact.
