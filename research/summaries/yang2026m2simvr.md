# M2SimVR: Multi-modal Mobility Simulation with VR, E-Scooter, Driving Simulator, SUMO, and Unity Integration
Zhiwei Yang, Saeed Mohammadian, Zuduo Zheng · unknown venue 2026 · https://doi.org/10.2139/ssrn.6341158 · relevance: low · basis: abstract

**Problem:** This paper presents a novel and flexible open-source simulation platform that integrates OpenStreetMap (OSM), SUMO (Simulation of Urban MObility), Unity 3D, Virtual Reality (VR), an electric scooter (e-scooter), and a Logitech driving wheel to create a realistic, interactive driving and riding simulator.
**Approach:** The platform supports multimodal traffic scenarios involving vehicles, e-scooters, and pedestrians. SUMO handles the microscopic traffic modeling of non-player vehicles and e-scooters, while Unity provides immersive 3D visualization with VR integration, pedestrian simulation, and real-time player inputs via physical devices, including an e-scooter handlebar (sensed by Arduino) and a Logitech driving wheel.
**Tools and scale:** Vehicular setting. The available text does not describe a V2X simulator, radio stack, traffic model or SCMS-style PKI in enough detail to report; scale not stated.
**Results:** SUMO handles the microscopic traffic modeling of non-player vehicles and e-scooters, while Unity provides immersive 3D visualization with VR integration, pedestrian simulation, and real-time player inputs via physical devices, including an e-scooter handlebar (sensed by Arduino) and a Logitech driving wheel.
**Limitations and future work (as stated by the authors):**
- None stated in the available text.
**Overlap with our engine:** none: simulation/digital twins in a vehicular context. Low relevance background for the simulator itself; it does not model the joint traffic/radio/SCMS loop, closed-loop revocation or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the integrated V2X case (SCMS scale, channel load, closed-loop safety impact) is not addressed.
