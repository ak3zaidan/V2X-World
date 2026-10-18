# A Corridor-Scale CARLA-VISSIM Co-Simulation Framework for Multi-Intersection Urban Traffic
Sima Ashayer, Austin Haris, Mina Sartipi · arXiv 2026 · https://arxiv.org/abs/2606.15431 · relevance: low · basis: abstract

**Problem:** This paper presents an implemented CARLA-VISSIM co-simulation framework for an urban corridor comprising approximately fifteen connected intersections centered on Martin Luther King Jr.
**Approach:** Boulevard in Chattanooga, Tennessee. The system integrates CARLA 0.10.0 Unreal Engine 5 with PTV VISSIM 2026 through a bidirectional, step-synchronized interface that couples VISSIM's microscopic vehicle, pedestrian, and signal-controller logic with CARLA's high-fidelity 3D rendering.
**Tools and scale:** Vehicular setting. The available text does not describe a V2X simulator, radio stack, traffic model or SCMS-style PKI in enough detail to report; scale not stated.
**Results:** The system integrates CARLA 0.10.0 Unreal Engine 5 with PTV VISSIM 2026 through a bidirectional, step-synchronized interface that couples VISSIM's microscopic vehicle, pedestrian, and signal-controller logic with CARLA's high-fidelity 3D rendering. A corridor-scale case study demonstrates consistent traffic-signal mirroring, synchronized vehicle-pedestrian interactions, and stable mixed-authority operation under peak loads of approximately 100 ...
**Limitations and future work (as stated by the authors):**
- The deployment captures the interaction of the five signalized intersections along MLK Street and their connecting upstream and downstream intersections, revealing synchronization challenges unique to multi-intersection corridors.
**Overlap with our engine:** none: simulation/digital twins in a vehicular context. Low relevance background for the simulator itself; it does not model the joint traffic/radio/SCMS loop, closed-loop revocation or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the integrated V2X case (SCMS scale, channel load, closed-loop safety impact) is not addressed.
