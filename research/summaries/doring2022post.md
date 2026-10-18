# Post-Quantum Cryptography in Use: Empirical Analysis of the TLS Handshake Performance
Ronny Doring, Marc Geitz · NOMS 2022-2022 IEEE/IFIP Network Operations and Management Symposium 2022 · https://doi.org/10.1109/noms54207.2022.9789913 · relevance: low · basis: abstract

**Problem:** With the increasing speed in the development of quantum computers, secure communication is at risk.
**Approach:** This paper presents an empirical analysis of how Post-Quantum Cryptography affects real-world performance compared to classical cryptography. Therefore we analyze the Transport Layer Security (TLS) handshake performance showing that Post-Quantum Cryptography can be as fast or even faster than classical cryptography, depending on the specific encryption and signature algorithms used.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** No quantitative results stated in the abstract.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — post-quantum/hybrid authentication scheme; the engine implements PQ and hybrid signatures, but the abstract states no SCMS-scale or traffic-coupled evaluation.
**Novelty implications:** We cannot claim PQ authentication for vehicles is new; the end-to-end safety cost of PQ in a full SCMS remains open as far as this abstract shows.
