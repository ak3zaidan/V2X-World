# GeDES: GPU-Driven Discrete Event Network Simulator
Qinyong Li, Zhi-Wei Zhao, Ge-Yong Min, Zi Wang, Luwei Fu · European Conference on Computer Systems 2026 · https://doi.org/10.1145/3767295.3769327 · relevance: low · basis: abstract

**Problem:** Discrete event network simulator (DES) is a fundamental service for the design, validation and optimization of various networked systems, including traditional computer networks and the recent LLM training/inference systems.
**Approach:** Efficient DES is strongly demanded by the community yet not delivered. The reason is: while DES allows for node-level parallelism, its concurrency potential has been significantly under-utilized (usually <5%) due to the limited number of CPU cores.
**Tools and scale:** Mentioned in abstract: simulation, simulator.
**Results:** In this paper, we aim to achieve ground-breaking performance improvement for DES by designing a novel GPU-driven simulation engine. Extensive simulation experiments show that GeDES significantly boosts DES by 33-2400X speedups compared to the SOTA works Unison and DONS.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — touches network/traffic simulation, LLM agents but not the joint traffic/radio/SCMS closed loop.
**Novelty implications:** Nothing in our candidate directions is ruled out; useful only as background on network/traffic simulation.
