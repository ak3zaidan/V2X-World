# AuNa: Modularly Integrated Simulation Framework for Cooperative Autonomous Navigation
Harun Teper, Anggera Bayuwindra, R. Riebl, R. Severino, Jian-Jia Chen, Kuan-Hsun Chen · arXiv.org 2022 · https://arxiv.org/abs/2207.05544 · relevance: low · basis: abstract

**Problem:** —In the near future, the development of autonomous driving will get more complex as the vehicles will not only rely on their own sensors but also communicate with other vehicles and the infrastructure to cooperate and improve the driving ex-perience.
**Approach:** Towards this, several research areas, such as robotics, communication, and control, are required to collaborate in order to implement future-ready methods. However, each area focuses on the development of its own components ﬁrst, while the effects the components may have on the whole system are only considered at a later stage.
**Tools and scale:** Vehicular setting. The available text does not describe a V2X simulator, radio stack, traffic model or SCMS-style PKI in enough detail to report; scale not stated.
**Results:** In this work, we integrate the simulation tools of robotics, communication and control namely ROS2, OMNeT++, and MATLAB to evaluate cooperative driving scenarios. We showcase the framework by demonstrating a platooning scenario under cooperative adaptive cruise control (CACC) and the ETSI ITS-G5 communication architecture.
**Limitations and future work (as stated by the authors):**
- —In the near future, the development of autonomous driving will get more complex as the vehicles will not only rely on their own sensors but also communicate with other vehicles and the infrastructure to cooperate and improve the driving ex-perience.
- Towards this, several research areas, such as robotics, communication, and control, are required to collaborate in order to implement future-ready methods.
**Overlap with our engine:** none: simulation/digital twins in a vehicular context. Low relevance background for the simulator itself; it does not model the joint traffic/radio/SCMS loop, closed-loop revocation or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the integrated V2X case (SCMS scale, channel load, closed-loop safety impact) is not addressed.
