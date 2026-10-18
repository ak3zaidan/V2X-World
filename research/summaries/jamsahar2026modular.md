# A Modular Platooning and Vehicle Coordination Simulator for Research and Education
Kevin Jamsahar, Adrian Wiltz, Maria Charitidou, Dimos V. Dimarogonas · 2026 34th Mediterranean Conference on Control and Automation (MED) 2026 · https://doi.org/10.1109/med70602.2026.11598251 · relevance: low · basis: abstract

**Problem:** This work presents a modular, Python-based simulator that simplifies the evaluation of novel vehicle control and coordination algorithms in complex traffic scenarios while keeping the implementation overhead low.
**Approach:** It allows researchers to focus primarily on developing the control and coordination strategies themselves, while the simulator manages the setup of complex road networks, vehicle configuration, execution of the simulation and the generation of video visualizations of the results. It is thereby also well-suited to support control education by allowing instructors to create interactive exercises providing students with direct visual feedback.
**Tools and scale:** Vehicular setting. The available text does not describe a V2X simulator, radio stack, traffic model or SCMS-style PKI in enough detail to report; scale not stated.
**Results:** GitHub: https://github.com/KTH-DHSG/Platooning_Simulator, Youtube: https://www.youtube.com/watch?v=7Ef_6DNhcoE
**Limitations and future work (as stated by the authors):**
- Thanks to its modular architecture, the simulator remains easily customizable and extensible, lowering the barrier for conducting advanced simulation studies in vehicle and traffic control research.
**Overlap with our engine:** none: simulation/digital twins in a vehicular context. Low relevance background for the simulator itself; it does not model the joint traffic/radio/SCMS loop, closed-loop revocation or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the integrated V2X case (SCMS scale, channel load, closed-loop safety impact) is not addressed.
