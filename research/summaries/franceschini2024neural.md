# A neural-network based anomaly detection system and a safety protocol to protect vehicular network
Marco Franceschini · arXiv (Cornell University) 2024 · https://arxiv.org/abs/2411.07013 · relevance: high · basis: full text

**Problem:** Detecting misbehaviour in platoons at run time.
**Approach:** LSTM MDS trained offline on VeReMi, deployed at run time in a platooning simulation with a defence protocol that dissolves the platoon on anomaly.
**Tools and scale:** MSc thesis; Plexe/Veins-style platoon simulation; VeReMi.
**Results:** Prevents nearly all accidents caused by misbehaviour; struggles to label specific attack types.
**Limitations and future work (as stated by the authors):**
- Difficulty of a universally adaptive protocol under varying traffic.
- More data and refinement needed before real-world use.
**Overlap with our engine:** strong — closed-loop detection inside a simulation with safety outcome (accidents), matching direction 1 (without PKI revocation).
**Novelty implications:** Closed-loop MBD-to-safety in simulation is done for platoons with a local response; MA-driven revocation and time-to-neutralise remain open.
