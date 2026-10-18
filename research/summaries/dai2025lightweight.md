# A Lightweight Key Agreement Protocol for V2X Communications Based on Kyber and Saber
Yinfei Dai, Qi Wang, Xiao Song, Shaoqiang Wang · Sensors 2025 · https://doi.org/10.3390/s25226938 · relevance: high · basis: full text

**Problem:** PQ key agreement for V2X must be lightweight.
**Approach:** Hybrid KEM handshake combining Kyber and Saber with RSU public-key broadcast and HMAC verification; NTT/AVX2 optimisations.
**Tools and scale:** Veins + SUMO, single OBU-RSU interactions.
**Results:** Lower handshake delay, faster key generation and lower resource use than RSA and ECDH.
**Limitations and future work (as stated by the authors):**
- Improve scalability via multithreading, batch authentication, cooperative edge computing
- extend to FrodoKEM and NTRU
- extend beyond single OBU-RSU to multi-node large-scale Veins/SUMO or NS-3 simulations
**Overlap with our engine:** partial — PQ KEM in Veins/SUMO, but only single-pair evaluation.
**Novelty implications:** Multi-node city-scale PQ evaluation explicitly left open.
