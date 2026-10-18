# Distributed Simulation for Digital Twins of Large-Scale Real-World DiffServ-Based Networks
Zhuoyao Huang, Nan Zhang, Jingran Shen, Georgios Diamantopoulos, Zhengchang Hua, Nikos Tziritas, Georgios Theodoropoulos · arXiv 2024 · https://doi.org/10.1007/978-3-031-69583-4_18 · relevance: low · basis: abstract

**Problem:** Digital Twin technology facilitates the monitoring and online analysis of large-scale communication networks.
**Approach:** Faster predictions of network performance thus become imperative, especially for analysing Quality of Service (QoS) parameters in large-scale city networks. Discrete Event Simulation (DES) is a standard network analysis technology, and can be further optimised with parallel and distributed execution for speedup, referred to as Parallel Discrete Event Simulation (PDES).
**Tools and scale:** Mentioned in abstract: simulation, simulator.
**Results:** A novel event-handling model for each network router is also proposed to significantly reduce the number of events in complex QoS modelling. Results show that compared to the conventional simulator OMNeT++/INET, even the sequential mode of Quaint can achieve a speedup of 53 times, and the distributed mode has a speedup of 232 times.
**Limitations and future work (as stated by the authors):**
- Scalability characterisation is conducted to portray the efficiency of distributed execution, and the results indicate the future direction for workload-aware model partitioning.
**Overlap with our engine:** partial — touches digital-twin modelling, network/traffic simulation but not the joint traffic/radio/SCMS closed loop.
**Novelty implications:** Nothing in our candidate directions is ruled out; useful only as background on digital-twin modelling.
