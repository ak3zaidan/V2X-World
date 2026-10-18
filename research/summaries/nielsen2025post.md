# Post-Quantum Digital Signature Algorithms on IoT: Evaluating Performance on LoRa ESP32 Microcontroller
Mads Nielsen, Magnus Kjeldsen, Togu Turnip, Birger Andersen · Proceedings of the 22nd International Conference on Security and Cryptography 2025 · https://doi.org/10.5220/0013508400003979 · relevance: low · basis: abstract

**Problem:** This research investigates practical considerations and challenges of deploying PQC digital signature algorithms on IoT devices.
**Approach:** We benchmarked the performance of CRYSTALS-Dilithium , Falcon , SPHINCS + across multiple NIST levels, measuring latency, memory usage and discussing parameter sizes. Additionally, we examined the communication overhead introduced by transmitting the larger-than-usual digital signatures over a wireless network.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** This study evaluates the feasibility of PQC algorithms with higher NIST security level parameters on a resource-constrained IoT device, the LoRa ESP32 microcontroller. Our findings reveal significant performance disparities between the tested algorithms, with Dilithium demonstrating the fastest execution and Falcon balancing speed and memory efficiency – even at higher NIST security levels.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — post-quantum/hybrid authentication scheme; the engine implements PQ and hybrid signatures, but the abstract states no SCMS-scale or traffic-coupled evaluation.
**Novelty implications:** We cannot claim PQ authentication for vehicles is new; the end-to-end safety cost of PQ in a full SCMS remains open as far as this abstract shows.
