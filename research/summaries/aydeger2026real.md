# Real-Time Cryptographic Agility for Autonomous UAV Swarms: A Performance-Driven Approach to Post-Quantum Migration
Abdullah Aydeger, Marco Carvalho · Consumer Communications and Networking Conference 2026 · https://doi.org/10.1109/ccnc65079.2026.11366545 · relevance: low · basis: abstract

**Problem:** This challenge is compounded by dynamic operational requirements: reconnaissance missions prioritize efficiency while tactical operations demand higher security, which static cryptographic configurations cannot accommodate.
**Approach:** The transition to post-quantum cryptography (PQC) introduces substantial computational overhead that can push cryptographic operations beyond coordination deadlines if algorithms are fixed a priori. We evaluate cryptographic agility, defined as adapting algorithms to operational context, using a simulator grounded in empirical cycle counts for ARM Cortex-M4 processors.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** Because no single NIST-approved PQC scheme jointly optimizes security, latency, and energy across varying mission requirements, these results provide evidence that adaptive selection is necessary to address performance bottlenecks in post-quantum UAV swarms.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — post-quantum/hybrid authentication scheme; the engine implements PQ and hybrid signatures, but the abstract states no SCMS-scale or traffic-coupled evaluation.
**Novelty implications:** We cannot claim PQ authentication for vehicles is new; the end-to-end safety cost of PQ in a full SCMS remains open as far as this abstract shows.
