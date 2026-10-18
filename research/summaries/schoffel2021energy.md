# On the Energy Costs of Post-Quantum KEMs in TLS-based Low-Power Secure IoT
Maximilian Schöffel, F. Lauer, Carl C. Rheinlander, N. Wehn · International Conference on Internet-of-Things Design and Implementation 2021 · https://doi.org/10.1145/3450268.3453528 · relevance: low · basis: abstract

**Problem:** Meanwhile, security is seen as one of the most critical issues of low-power IoT devices even with pre-quantum public key cryptography since IoT devices have strict energy constraints and limited computational power.
**Approach:** In this paper, we carried out an in-depth investigation of the application of potential Post-Quantum Cryptography algorithms, which were proposed in the associated US NIST process, to a representative TLS-based low-power IoT infrastructure. First, we show that the main contributor to the TLS handshake latency are the higher bandwidth requirements of post-quantum Key-Encapsulation Mechanisms rather than the cryptographic computations itself.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** It is common belief that the complexity of the cryptographic computations are also the bottleneck of the new, quantum-resistant algorithms and that hardware accelerators are necessary to use them efficiently on energy constrained embedded devices. Second, from the perspective of crypto-agility we show that edge devices with code-based, isogeny-based as well as lattice-based algorithms have low energy consumption, which enables long battery run times in typical IoT scenarios without dedicated hardware accelerators.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — post-quantum/hybrid authentication scheme; the engine implements PQ and hybrid signatures, but the abstract states no SCMS-scale or traffic-coupled evaluation.
**Novelty implications:** We cannot claim PQ authentication for vehicles is new; the end-to-end safety cost of PQ in a full SCMS remains open as far as this abstract shows.
