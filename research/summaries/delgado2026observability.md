# Observability for Post-Quantum TLS Readiness: A Multi-Surface Evidence Framework
J. L. Delgado · IACR Cryptology ePrint Archive 2026 · https://arxiv.org/abs/2605.02978 · relevance: low · basis: abstract

**Problem:** Against an inherited TLS quantum-vulnerability analyzer, the baseline detects 2 of 29 runs and 0 of 23 TLS 1.3 runs.
**Approach:** We present a multi-surface framework for post-quantum TLS observability. We instantiate it as a reproducible artifact with schema-enforced observations and results, versioned registries, auditable inference rules, stress contracts, and baseline adapters.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** This distinction is essential under TLS 1.3 encryption, resumption, mutual TLS, trace truncation, fragmentation, coalescing, active certificate retrieval, and temporal drift. We evaluate the framework on 29 controlled scenarios spanning TLS 1.2 and TLS 1.3, classical and hybrid key establishment, mutual TLS, resumption, HelloRetryRequest, truncation, fragmentation and coalescing, temporal drift, IPv6, and chain-depth variation.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — post-quantum/hybrid authentication scheme; the engine implements PQ and hybrid signatures, but the abstract states no SCMS-scale or traffic-coupled evaluation.
**Novelty implications:** We cannot claim PQ authentication for vehicles is new; the end-to-end safety cost of PQ in a full SCMS remains open as far as this abstract shows.
