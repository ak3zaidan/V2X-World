# MultiVehicle Simulator (MVSim): lightweight dynamics simulator for multiagents and mobile robotics research
José-Luis Blanco-Claraco, Borys Tymchenko, Francisco José Mañas-Alvarez, Fernando Cañadas-Aránega et al. · arXiv 2023 · https://doi.org/10.1016/j.softx.2023.101443. · relevance: low · basis: abstract

**Problem:** Development of applications related to closed-loop control requires either testing on the field or on a realistic simulator, with the latter being more convenient, inexpensive, safe, and leading to shorter development cycles.
**Approach:** To address that need, the present work introduces MVSim, a simulator for multiple vehicles or robots capable of running dozens of agents in simple scenarios, or a handful of them in complex scenarios. MVSim employs realistic physics-grounded friction models for tire-ground interaction, and aims at accurate and GPU-accelerated simulation of most common modern sensors employed in mobile robotics and autonomous vehicle research, such as depth and RGB cameras, or 2D and 3D LiDAR scanners.
**Tools and scale:** Mentions: simulation (details not in abstract).
**Results:** Efficient simulation is achieved by means of focusing on ground vehicles, which allows the use of a simplified 2D physics engine for body collisions while solving wheel-ground interaction forces separately.
**Limitations and future work (as stated by the authors):**
- None stated in the available text.
**Overlap with our engine:** none — adjacent background; does not implement simulation, SCMS backend, or closed-loop evaluation comparable to the engine (judged from abstract).
**Novelty implications:** Rules out no candidate-direction claim on the available evidence; background reference only.
