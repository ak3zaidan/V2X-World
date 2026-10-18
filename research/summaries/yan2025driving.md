# Driving into the Unknown: Investigating and Addressing Security Breaches in Vehicle Infotainment Systems
Minrui Yan, George Crane, Dean Sullivan, Haoqi Shan · Sensors 2025 · https://doi.org/10.3390/s26010077 · relevance: medium · basis: abstract

**Problem:** Hardcoded credentials in IVI firmware enabling fleet-scale backend compromise.
**Approach:** Cross-layer framework: firmware extraction, symbolic execution, targeted fuzzing of IVI-to-backend interactions.
**Tools and scale:** Seven real automotive systems (Mercedes-Benz, Tesla, SAIC, FAW-VW, Denza, Bosch, Minieye).
**Results:** Uncovered SSRF and broken access control; a full remote-control exploit demonstrated.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — Real-world IVI/backend security, not V2X PKI simulation.
**Novelty implications:** No effect on our novelty claims; background only.
