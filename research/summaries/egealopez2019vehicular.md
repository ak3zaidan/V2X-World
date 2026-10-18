# Vehicular Networks Simulation With Realistic Physics
Esteban Egea-Lopez, Fernando Losilla, Juan Pascual-Garcia, Jose Maria Molina-Garcia-Pardo · IEEE Access 2019 · https://doi.org/10.1109/access.2019.2908651 · relevance: high · basis: full text

**Problem:** Cooperative automated driving evaluation needs realistic vehicle/traffic dynamics and communications.
**Approach:** Veneris (Unity game-engine traffic simulator with realistic vehicle model; IDM and MOBIL adapted to 3D; cost-based lane change) plus Opal GPU ray tracer for propagation; duplicate-ray filtering.
**Tools and scale:** Unity, GPU ray tracing; validated against specialised tools.
**Results:** Realistic results in vehicle/traffic dynamics and propagation that match specialised tools.
**Limitations and future work (as stated by the authors):**
- Future: evaluate influence of physically accurate models on traffic results
- implement diffraction and validate with measurements
- performance optimisation and interactive simulation.
**Overlap with our engine:** strong — strong: a game-engine-based 3D traffic + V2X simulator with IDM/MOBIL and ray-traced radio — overlaps our V2 2D/3D world-simulator vision and our IDM/MOBIL traffic.
**Novelty implications:** Cannot claim first 3D game-engine V2X simulator with IDM/MOBIL; open: PKI, security and determinism in such a 3D world.
