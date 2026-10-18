# IoT-Simulated Digital Twin with AI Traffic Signal Control for Real-Time Traffic Optimization in SUMO
Vasilica Cerasela Doiniţa Ceapă, Vasile Alexandru Apostol, Ioan Stefan Sacală, Constantin Florin Căruntu, Russ Ross, Dj Holt et al. · Sensors 2026 · https://doi.org/10.3390/s26061880 · relevance: low · basis: abstract

**Problem:** Urban traffic congestion leads to longer travel times, economic losses, and increased pollution.
**Approach:** Recent advances in the Internet of Things (IoT) provide detailed real-time traffic data, yet testing adaptive control strategies directly on live networks remains costly and risky. To address this challenge, we propose an IoT-driven digital twin framework for the design and evaluation of AI-based traffic management systems.
**Tools and scale:** Vehicular setting. The available text does not describe a V2X simulator, radio stack, traffic model or SCMS-style PKI in enough detail to report; scale not stated.
**Results:** The framework is implemented in the Simulation of Urban MObility (SUMO) and uses its Python 3.14.2 API to emulate a dense network of IoT sensors that stream real-time information on vehicle density, queue lengths, and waiting times.
**Limitations and future work (as stated by the authors):**
- Recent advances in the Internet of Things (IoT) provide detailed real-time traffic data, yet testing adaptive control strategies directly on live networks remains costly and risky.
- To address this challenge, we propose an IoT-driven digital twin framework for the design and evaluation of AI-based traffic management systems.
**Overlap with our engine:** none: simulation/digital twins in a vehicular context. Low relevance background for the simulator itself; it does not model the joint traffic/radio/SCMS loop, closed-loop revocation or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the integrated V2X case (SCMS scale, channel load, closed-loop safety impact) is not addressed.
