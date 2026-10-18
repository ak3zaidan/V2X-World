# Flowsim: A Modular Simulation Platform for Microscopic Behavior Analysis of City-Scale Connected Autonomous Vehicles
Ye Tao, Ehsan Javanmardi, Jin Nakazato, Manabu Tsukada, Hiroshi Esaki · 2023 IEEE 26th International Conference on Intelligent Transportation Systems (ITSC) 2023 · https://doi.org/10.1109/itsc57777.2023.10421900 · relevance: high · basis: full text

**Problem:** Existing simulators lack flexibility to represent CAV OBU behaviour at city scale.
**Approach:** Flowsim: modular pure-Python (~1,500 LOC) city-scale simulator for CAV behaviour and data flow, driven by SUMO scenarios; supports cybersecurity vulnerability assessment.
**Tools and scale:** Python (PyPy), SUMO; realistic traffic scenarios.
**Results:** Functionality and performance verified (no numbers extracted).
**Limitations and future work (as stated by the authors):**
- PyPy C-extension calls hurt performance
- GIL limits
- future: advanced perception, detailed communication protocols, planning and control.
**Overlap with our engine:** partial — city-scale CAV data-flow simulator with cybersecurity evaluation, but without detailed radio or PKI.
**Novelty implications:** City-scale CAV security-oriented simulators exist at abstract level; ours is distinct in protocol depth.
