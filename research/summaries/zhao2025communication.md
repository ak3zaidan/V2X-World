# A Communication-Latency-Aware Co-Simulation Platform for Safety and Comfort Evaluation of Cloud-Controlled ICVs
Yongqi Zhao, Xinrui Zhang, Tomislav Mihalj, Martin Schabauer, Luis Putzer, Erik Reichmann-Blaga et al. · IEEE Internet of Things Journal 2025 2025 · https://doi.org/10.1109/jiot.2025.3634326 · relevance: high · basis: full text

**Problem:** Testing cloud-controlled ICVs needs realistic V2C latency plus vehicle and traffic simulation.
**Approach:** CarMaker + Vissim co-simulation with Gamma-distributed latency from 5G measurements (China, Hungary) and a proactive conflict module.
**Tools and scale:** CarMaker, Vissim; 8 test conditions.
**Results:** PCM gives about fivefold more collisions, threefold more critical following, up to 21-fold critical cut-in; latency mainly reduces comfort and aggravates safety under extreme conditions.
**Limitations and future work (as stated by the authors):**
- PCM slows simulation four- to fivefold
- future: event-triggered updates, efficient interfaces, HPC deployment, OpenSCENARIO critical scenarios, V2I cooperative perception.
**Overlap with our engine:** partial — communication latency to safety outcome evaluation (collision rate, PET).
**Novelty implications:** Communication-impairment-to-safety-metrics simulation exists; security-specific delay remains.
