# Elevation Aware 2D/3D Co-simulation Framework for Large-scale Traffic Flow and High-fidelity Vehicle Dynamics
Chandra Raskoti, Weizi Li · arXiv 2025 · https://arxiv.org/abs/2512.11249 · relevance: low · basis: abstract

**Problem:** Reliable testing of autonomous driving systems requires simulation environments that combine large-scale traffic modeling with realistic 3D perception and terrain.
**Approach:** Existing tools rarely capture real-world elevation, limiting their usefulness in cities with complex topography. This paper presents an automated, elevation-aware co-simulation framework that integrates SUMO with CARLA using a pipeline that fuses OpenStreetMap road networks and USGS elevation data into physically consistent 3D environments.
**Tools and scale:** Vehicular setting. The available text does not describe a V2X simulator, radio stack, traffic model or SCMS-style PKI in enough detail to report; scale not stated.
**Results:** This paper presents an automated, elevation-aware co-simulation framework that integrates SUMO with CARLA using a pipeline that fuses OpenStreetMap road networks and USGS elevation data into physically consistent 3D environments. The system generates smooth elevation profiles, validates geometric accuracy, and enables synchronized 2D-3D simulation across platforms.
**Limitations and future work (as stated by the authors):**
- None stated in the available text.
**Overlap with our engine:** none: simulation/digital twins in a vehicular context. Low relevance background for the simulator itself; it does not model the joint traffic/radio/SCMS loop, closed-loop revocation or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the integrated V2X case (SCMS scale, channel load, closed-loop safety impact) is not addressed.
