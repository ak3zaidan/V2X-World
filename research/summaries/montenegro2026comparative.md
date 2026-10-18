# Comparative analysis of post-quantum handshake performance in QUIC and TLS protocols
José A. Montenegro, Ruben Rios, Javier Bonilla · Computer Networks 2026 · https://doi.org/10.1016/j.comnet.2025.111957 · relevance: low · basis: abstract

**Problem:** The rapid advancement of quantum computing threatens the security foundations of today’s communication protocols.
**Approach:** Consequently, in this work we evaluate their performance when combined with post-quantum primitives. To this end, we devise a practical evaluation framework that integrates the cryptographic primitives into TLS and QUIC implementations.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** On the web, the traditional approach to securing web traffic has been to couple HTTP with TLS over TCP, but QUIC over UDP has recently emerged as an alternative to reduce latency and improve performance over unreliable networks. Moreover, QUIC consistently reduces the performance penalty of post-quantum primitives compared to TLS, especially in lossy network environments.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — post-quantum/hybrid authentication scheme; the engine implements PQ and hybrid signatures, but the abstract states no SCMS-scale or traffic-coupled evaluation.
**Novelty implications:** We cannot claim PQ authentication for vehicles is new; the end-to-end safety cost of PQ in a full SCMS remains open as far as this abstract shows.
