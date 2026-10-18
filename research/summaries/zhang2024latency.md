# Latency Estimation and Computational Task Offloading in Vehicular Mobile Edge Computing Applications
Wenhan Zhang, Mingjie Feng, Marwan Krunz · IEEE Transactions on Vehicular Technology 2024 · https://doi.org/10.1109/tvt.2023.3334192 · relevance: medium · basis: abstract

**Problem:** Mobile edge computing (MEC) is a key enabler of time-critical vehicle-to-everything (V2X) applications. Under MEC, a vehicle has the option to offload computationally intensive tasks to a nearby edge server or to a remote cloud server.
**Approach:** In this paper, we first conduct extensive measurements of the round-trip time (RTT) between a vehicular user and edge/cloud servers. Using these measurements, we present a latency-estimation framework for optimal task offloading.
**Tools and scale:** Not stated in the abstract. (From the abstract only.)
**Results:** Determining where to execute a task necessitates accurate estimation of the end-to-end (E2E) offloading delay. Predicted propagation delays are incorporated into virtual simulations to estimate the transmission, queuing, and processing delays, hence accounting for the E2E delay. Based on the estimated E2E delay, we design a task offloading scheme that minimizes the offloading latency while maintaining a low packet loss rate.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** none — Adjacent background (learned prediction of V2X/vehicular link or QoS performance); no direct overlap with the engine's traffic+radio+SCMS joint simulation.
**Novelty implications:** Does not restrict any candidate claim.
