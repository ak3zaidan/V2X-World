# RVE-CV2X: A Scalable Emulation Framework for Real-Time Evaluation of CV2X-Based Connected Vehicle Applications
Ghayoor Shah, MD Saifuddin, Yaser P. Fallah, Somak Datta Gupta · arXiv 2021 · https://doi.org/10.1109/vnc51378.2020.9318345 · relevance: high · basis: full text

**Problem:** Full C-V2X simulators are much slower than real time; need scalable real-time testing.
**Approach:** RVE-CV2X real-time HIL emulator of C-V2X Mode 4 (MAC/PHY of emulated remote vehicles from the host vehicle's perspective), validated against an ns-3 C-V2X simulator.
**Tools and scale:** RTCSim-CV2X on a Linux PC (i7-6700, 32 GB), NIST error model, real OBU.
**Results:** Accuracy verified against validated ns-3 C-V2X simulator (no numbers extracted).
**Limitations and future work (as stated by the authors):**
- OBU memory/processor limits forced running the emulator on a PC.
**Overlap with our engine:** partial — real-time C-V2X Mode 4 emulation; no security.
**Novelty implications:** Real-time C-V2X emulation exists.
