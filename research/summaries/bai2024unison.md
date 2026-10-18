# Unison: A Parallel-Efficient and User-Transparent Network Simulation Kernel
Songyuan Bai, Hao Zheng, Chen Tian, Xiaoliang Wang, Chang Liu, Xin Jin, Fu Xiao, Qiao Xiang, Wanchun Dou, Guihai Chen · Proceedings of the Nineteenth European Conference on Computer Systems 2024 · https://doi.org/10.1145/3627703.3629574 · relevance: low · basis: abstract

**Problem:** Discrete-event simulation (DES) is a prevalent tool for evaluating network designs.
**Approach:** Although DES offers full fidelity and generality, its slow performance limits its application. To speed up DES, many network simulators employ parallel discrete-event simulation (PDES).
**Tools and scale:** Mentioned in abstract: ns-3, simulation, simulators.
**Results:** However, adapting existing network simulation models to PDES requires complex reconfigurations and often yields limited performance improvement. Testbed experiments on commodity servers demonstrate that Unison can achieve a 40× speedup over DES using 24 CPU cores, and a 10× speedup compared with existing PDES algorithms under the same CPU cores.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — touches network/traffic simulation but not the joint traffic/radio/SCMS closed loop.
**Novelty implications:** Nothing in our candidate directions is ruled out; useful only as background on network/traffic simulation.
