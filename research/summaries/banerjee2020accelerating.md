# Accelerating Post-Quantum Cryptography using an Energy-Efficient TLS Crypto-Processor
Utsav Banerjee, Siddharth Das, Anantha P. Chandrakasan · 2020 IEEE International Symposium on Circuits and Systems (ISCAS) 2020 · https://doi.org/10.1109/iscas45731.2020.9180550 · relevance: low · basis: abstract

**Problem:** Post-quantum cryptography (PQC) is currently a growing area of research and NIST PQC Round 2 schemes are being actively analyzed and optimized for both security and efficiency.
**Approach:** In this work, we repurpose the cryptographic accelerators in an energy-efficient pre-quantum TLS crypto-processor to implement post-quantum key encapsulation schemes SIKE, Frodo and ThreeBears and signature scheme SPHINCS+. We utilize the modular arithmetic unit inside the elliptic curve cryptography accelerator to implement SIKE, while we use the AES-256 and SHA2-256 hardware primitives to substitute SHA3-256 and SHAKE-256 computations and accelerate the other three protocols.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** We accelerate the most computationally expensive components of these PQC protocols in hardware, thereby achieving up to an order of magnitude improvement in energy-efficiency over software implementations.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — post-quantum/hybrid authentication scheme; the engine implements PQ and hybrid signatures, but the abstract states no SCMS-scale or traffic-coupled evaluation.
**Novelty implications:** We cannot claim PQ authentication for vehicles is new; the end-to-end safety cost of PQ in a full SCMS remains open as far as this abstract shows.
