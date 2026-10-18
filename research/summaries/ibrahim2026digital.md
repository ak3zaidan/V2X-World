# Digital Twin for Ultra-Reliable & Low-Latency 6G Wireless Communications in Dense Urban City
Abdikarim Mohamed Ibrahim, Rosdiadee Nordin · arXiv 2026 · https://arxiv.org/abs/2601.07132 · relevance: low · basis: abstract

**Problem:** High-frequency deployments in dense cities are difficult to plan because coverage, interference, and service reliability depend sensitively on local morphology.
**Approach:** This paper develops a geometric Digital Twin (DT) of the Sunway City and uses it to study the service implications of a multi-site mmWave deployment. The DT is constructed from geo-referenced three-dimensional meshes of buildings, roads, and open areas, assembled in Blender and exported as a mesh scene.
**Tools and scale:** Vehicular setting. The available text does not describe a V2X simulator, radio stack, traffic model or SCMS-style PKI in enough detail to report; scale not stated.
**Results:** A seven-transmitter downlink at 10 GHz is then embedded into this geometry and evaluated using a GPU accelerated ray tracing engine that returns path-gain and Signal-to-Interference-plus-Noise Ratio (SINR) fields over a dense grid of user locations. These fields are mapped to achievable throughput and compared against representative target rates for immersive extended reality (XR), vehicle-to-everything (V2X) services, and ultra-reliable ...
**Limitations and future work (as stated by the authors):**
- None stated in the available text.
**Overlap with our engine:** none: simulation/digital twins in a vehicular context. Low relevance background for the simulator itself; it does not model the joint traffic/radio/SCMS loop, closed-loop revocation or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the integrated V2X case (SCMS scale, channel load, closed-loop safety impact) is not addressed.
