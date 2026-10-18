# FTO-Sim: an open-source simulation framework for evaluating cooperative perception in urban areas
Mario Ilic, Tanja Niels, Mathias Pechinger, Klaus Bogenberger · European Transport Research Review 2026 · https://doi.org/10.1186/s12544-026-00802-8 · relevance: high · basis: full text

**Problem:** Occlusions impair VRU detection; cooperative perception evaluation needs realistic occlusion.
**Approach:** FTO-Sim: SUMO + Python ray-tracing occlusion module; floating car and floating bike observers; visibility metrics.
**Tools and scale:** SUMO, Python ray tracing; replications of published studies.
**Results:** Replicates published studies (qualitative).
**Limitations and future work (as stated by the authors):**
- Relative visibility not comparable across scenarios
- LoV metric needs demand-dependent calibration and observer velocity
- efficiency and motion-planning applications not included
- dependence on incomplete OSM and simplified SUMO representations.
**Overlap with our engine:** none — perception, no V2X stack.
**Novelty implications:** No impact.
