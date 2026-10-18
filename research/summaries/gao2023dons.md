# DONS: Fast and Affordable Discrete Event Network Simulation with Automatic Parallelization
Kaihui Gao, Li Chen, Dan Li, Vincent Liu, Xizheng Wang, Ran Zhang, Lu Lu · Proceedings of the ACM SIGCOMM 2023 Conference 2023 · https://doi.org/10.1145/3603269.3604844 · relevance: low · basis: abstract

**Problem:** Discrete Event Simulation (DES) is an essential tool for network practitioners.
**Approach:** Unfortunately, existing DES simulators cannot achieve satisfactory performance at the scale of modern networks. Recent work has attempted to address these challenges by reducing the traffic processed via novel approximation techniques; however, we argue in this paper that much of the slowdown of existing DES simulators is due to their underlying software architecture.
**Tools and scale:** Mentioned in abstract: simulate, simulation, simulator, simulators.
**Results:** DONS can automatically parallelize simulation within and across servers to achieve high core utilization, low cache miss rate, and high memory efficiency. On a cluster of CPU-based servers, DONS can achieve a speedup of 65×, matching the order of magnitude of recent GPU-accelerated deep learning performance estimators, but without any loss of accuracy.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — touches network/traffic simulation but not the joint traffic/radio/SCMS closed loop.
**Novelty implications:** Nothing in our candidate directions is ruled out; useful only as background on network/traffic simulation.
