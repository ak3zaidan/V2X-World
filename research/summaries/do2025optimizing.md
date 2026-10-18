# Optimizing packet size in post-quantum NB-IoT systems: Signature aggregation and Merkle tree pruning approaches
Thi Bac Do, Khanh Linh Dinh · Journal of Computer Science and Cybernetics 2025 · https://doi.org/10.15625/1813-9663/23150 · relevance: low · basis: abstract

**Problem:** With the increasing adoption of post-quantum cryptographic schemes such as SPHINCS+ in Narrowband IoT (NB-IoT) systems, a major challenge arises from large packet size overhead - often reaching approximate 34 KB per transmission due to stateless hash-based signature metadata.
**Approach:** This paper investigates practical techniques for reducing this overhead while maintaining quantum resilience, including signature aggregation across multiple devices, Merkle tree pruning within SPHINCS+, and selective signing policies based on session frequency. We implement these strategies on an ESP32-WROOM-32 microcontroller interfaced with a SIM7080 NB-IoT module, demonstrating that packet size can be reduced by up to 60% through intelligent session management and optimized scheduling.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** No quantitative results stated in the abstract.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — post-quantum/hybrid authentication scheme; the engine implements PQ and hybrid signatures, but the abstract states no SCMS-scale or traffic-coupled evaluation.
**Novelty implications:** We cannot claim PQ authentication for vehicles is new; the end-to-end safety cost of PQ in a full SCMS remains open as far as this abstract shows.
