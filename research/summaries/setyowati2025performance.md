# Performance Evaluation of Hybrid TLS with ML-KEM-768 for Post-Quantum Secure MQTT in Mobile IIoT Systems
Danny Setyowati, H. A. Danang Rimbawa, Achmad Farid Wadjdi · 2025 IEEE 11th International Conference on Computing, Engineering and Design (ICCED) 2025 · https://doi.org/10.1109/icced68324.2025.11324853 · relevance: low · basis: abstract

**Problem:** The imminent threat of quantum computing necessitates the integration of Post-Quantum Cryptography (PQC) to secure critical infrastructure, particularly within mobile Industrial Internet of Things (IIoT) environments.
**Approach:** A virtualized testbed was developed to simulate a network of Mobile Manufacturing Trucks (MMTs), acting as IIoT clients using Node-RED, transmitting real-time sensor data to a central Security Operations Center (SOC) running a Mosquitto MQTT broker.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** The security of the data channel is enhanced via an applicationlayer hybrid scheme that augments a standard TLS connection with the NIST-standardized ML-KEM-768 key encapsulation mechanism, orchestrated by distributed C-based cryptographic services. The mean end-to-end responsiveness was measured at 108.80 ms, successfully meeting the sub- 300 ms requirement for real-time industrial applications.
**Limitations and future work (as stated by the authors):**
- "Analysis of the latency distribution revealed a realistic network jitter profile, with a small number of outliers, while the majority of transmissions remained well below 150 ms."
- "Furthermore, CPU utilization on all virtual machines remained minimal, confirming the computational efficiency of ML-KEM for resource-constrained edge devices."
- "This work establishes a practical blueprint for deploying quantum-resistant protocols, with future research directed toward hardware-based validation, the integration of PQC digital signatures, and AI-driven anomaly detection."
**Overlap with our engine:** partial — post-quantum/hybrid authentication scheme; the engine implements PQ and hybrid signatures, but the abstract states no SCMS-scale or traffic-coupled evaluation.
**Novelty implications:** We cannot claim PQ authentication for vehicles is new; the end-to-end safety cost of PQ in a full SCMS remains open as far as this abstract shows.
