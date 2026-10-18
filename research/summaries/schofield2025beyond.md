# Beyond Simulation: Benchmarking World Models for Planning and Causality in Autonomous Driving
Hunter Schofield, Mohammed Elmahgiubi, Kasra Rezaee, Jinjun Shan · arXiv 2025 · https://arxiv.org/abs/2508.01922 · relevance: high · basis: full text (extracted sections)

**Problem:** Are metrics for world models as traffic simulators suitable for policy training?
**Approach:** Analyse WOSAC metametric; partial replay; extend evaluation to agents causal to the ego; new sensitivity metrics.
**Tools and scale:** Waymo Open Sim-Agents Challenge data; state-of-the-art world models.
**Results:** Many top models fail when the ego is forced to replay original trajectory.
**Limitations and future work (as stated by the authors):**
- Standard metrics do not capture causal sensitivity.
**Overlap with our engine:** partial - causal evaluation of learned traffic world models (direction 3), no networking.
**Novelty implications:** Causal evaluation of driving world models exists; network-and-security world models remain open.
