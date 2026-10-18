# Simulating Hybrid Aerial- and Ground-based Vehicular Networks with ns-3 and LIMoSim
Benjamin Sliwa, Manuel Patchou, Karsten Heimann, Christian Wietfeld · arXiv 2020 · https://doi.org/10.1145/3389400.3389407 · relevance: high · basis: full text

**Problem:** Simulating hybrid aerial and ground vehicular networks in a controllable environment.
**Approach:** LIMoSim open-source framework integrated into ns-3 via shared codebase coupling in a single simulation process.
**Tools and scale:** ns-3, LIMoSim analytical mobility, C-V2X and mmWave case studies.
**Results:** Case studies show integration for C-V2X and mmWave (LTE vs mmWave data rate/delay under load).
**Limitations and future work (as stated by the authors):**
- Integrating reinforcement-learning mobility control and networking
- combining with Data-driven Network Simulation (DDNS)
- online visualisation of communication.
**Overlap with our engine:** partial — single-process coupling of mobility and network simulation, like our design; no PKI.
**Novelty implications:** Single-process mobility+network coupling is prior art; we cannot claim it as novel.
