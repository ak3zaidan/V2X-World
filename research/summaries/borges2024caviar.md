# CAVIAR: Co-Simulation of 6G Communications, 3-D Scenarios, and AI for Digital Twins
João Borges, Felipe Bastos, Ilan Correa, Pedro Batista et al. · IEEE Internet of Things Journal 2024 · https://doi.org/10.1109/jiot.2024.3418675 · relevance: medium · basis: abstract

**Problem:** Digital twins are an important technology for advancing mobile communications, specially in use cases that require simultaneously simulating the wireless channel, 3-D scenes and machine learning (ML). Aiming at contributing towards a solution to this demand, this work describes a modular co-simulation methodology called CAVIAR, for implementing the virtual counterpart of a digital twin (DT) system.
**Approach:** The main contributions of this work are the detailed description of different CAVIAR architectures, the implementation of this methodology to assess a 6G use case of unmanned aerial vehicle (UAV)-based search and rescue (SAR), and the generation of benchmarking data about the computational resource usage. For executing the SAR co-simulation we adopt five open-source solutions: 1) the physical and link level network simulator Sionna; 2) the simulator for autonomous vehicles AirSim; 3) scikit-learn for training a decision tree for multiple input–multiple output (MIMO) beam selection; 4) Yolov8 for the detection of rescue targets; and 5) neural autonomic transport system (NATS) for message passing.
**Tools and scale:** Mentioned in abstract: Sionna, AirSim, 6G, digital twin. (From the abstract only.)
**Results:** Here, CAVIAR is upgraded to support a message passing library and facilitate using different 6G-related simulators.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** partial — CAVIAR co-simulation of 6G comms, 3D scenes and AI.
**Novelty implications:** Same.
