# PQTLS-AD: Post-Quantum TLS Accelerated with DNS
Sangwon Lim, Hyeonmin Lee, Gyeongheon Jeong, Ted Taekyoung Kwon · 2025 34th International Conference on Computer Communications and Networks (ICCCN) 2025 · https://doi.org/10.1109/icccn65249.2025.11133832 · relevance: low · basis: abstract

**Problem:** Transport Layer Security (TLS) is expected to transition to Post-Quantum Cryptography (PQC) to mitigate the threats posed by quantum computing.
**Approach:** In this paper, we propose PQTLS-AD, a novel method that leverages the Domain Name System (DNS) to efficiently distribute PQC certificates. We have developed a PQTLS-AD prototype and conducted extensive experiments.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** By offloading certificate transmission to DNS, PQTLS-AD reduces handshake data overhead, effectively lowering handshake latency. Our results show that PQTLS-AD significantly reduces handshake latency, cutting it by more than one Round-Trip Time (RTT) compared to standard PQTLS.
**Limitations and future work (as stated by the authors):**
- "Various solutions have been proposed to address this issue; however, they suffer from limitations, such as the necessity of pre-fetching certificates or dependency on prior connections."
**Overlap with our engine:** partial — post-quantum/hybrid authentication scheme; the engine implements PQ and hybrid signatures, but the abstract states no SCMS-scale or traffic-coupled evaluation.
**Novelty implications:** We cannot claim PQ authentication for vehicles is new; the end-to-end safety cost of PQ in a full SCMS remains open as far as this abstract shows.
