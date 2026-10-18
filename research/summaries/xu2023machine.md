# Machine Learning for Interconnect Network Traffic Forecasting: Investigation and Exploitation
Xiongxiao Xu, Xin Wang, Elkin Cruz-Camacho, Christopher D. Carothers, Kevin A. Brown, Robert B. Ross, Zhiling Lan, Kai Shu · ACM SIGSIM Conference on Principles of Advanced Discrete Simulation 2023 · https://doi.org/10.1145/3573900.3591123 · relevance: low · basis: abstract

**Problem:** Interconnect networks play a key role in high-performance computing (HPC) systems.
**Approach:** Parallel discrete event simulation (PDES) has been a long-standing pillar for studying large-scale networking systems by replicating the real-world behaviors of HPC facilities. However, the simulation requirements and computational complexity of PDES are growing at an intractable rate.
**Tools and scale:** Mentioned in abstract: simulation.
**Results:** An active research topic is to build a surrogate-ready PDES framework where an accurate surrogate model built on machine learning can be used to forecast network traffic for improving PDES. Our preliminary analysis demonstrates promising results and shows that ADP-LSTM can consistently outperform ARIMA with significantly less time overhead.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — touches network/traffic simulation, learned surrogates but not the joint traffic/radio/SCMS closed loop.
**Novelty implications:** Nothing in our candidate directions is ruled out; useful only as background on network/traffic simulation.
