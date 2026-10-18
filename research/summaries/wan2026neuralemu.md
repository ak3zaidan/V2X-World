# NeuralEmu: in situ Measurement-Driven, ML-based, High-Fidelity 5G Network Emulation
Haoran Wan, Yaxiong Xie, Kyle Jamieson · arXiv.org 2026 · https://arxiv.org/abs/2604.26080 · relevance: high · basis: full text (extracted sections)

**Problem:** Record-replay emulators break feedback; simulators use simplistic schedulers.
**Approach:** NeuralEmu: ML-learned commercial 5G scheduler behaviour plus traffic reconstruction for background users; Linux middlebox emulator.
**Tools and scale:** High-resolution telemetry from T-Mobile; indoor stationary UEs.
**Results:** ABR bitrate error from 7% to below 1%; over 80% reduction in congestion-control RTT error; 55% better page load time emulation.
**Limitations and future work (as stated by the authors):**
- Stationary UEs; mobility future work.
- Downlink only.
- Single operator (T-Mobile).
**Overlap with our engine:** none.
**Novelty implications:** No effect.
