# quantum-safe: Bridging the Post-Quantum Production Gap with a Hybrid-by-Default Python Cryptography Library
Animesh Shaw · arXiv.org 2026 · https://arxiv.org/abs/2605.17061 · relevance: low · basis: abstract

**Problem:** FIPS 203, 204 and 205 closed the algorithmic gap in post-quantum cryptography (PQC); the production gap -- hybrid combiners, conformance evidence, migration tooling, stateful signing, protocol helpers -- remains open.
**Approach:** A methodology that scores it must not choose its own dimensions, so we anchor nine to external requirements (CNSA 2.0, CMVP, the TNO CADI survey, the IETF hybrid draft). We present quantum-safe, a hybrid-by-default Python library built against this rubric, and an audit of nine PQC libraries (September 2026).
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** The exercise can lower our own score, and it does. Hybrid combiners exist in three of six verifiable libraries, not the one a March-2026 snapshot found.
**Limitations and future work (as stated by the authors):**
- "FIPS 203, 204 and 205 closed the algorithmic gap in post-quantum cryptography (PQC); the production gap -- hybrid combiners, conformance evidence, migration tooling, stateful signing, protocol helpers -- remains open."
**Overlap with our engine:** partial — post-quantum/hybrid authentication scheme; the engine implements PQ and hybrid signatures, but the abstract states no SCMS-scale or traffic-coupled evaluation.
**Novelty implications:** We cannot claim PQ authentication for vehicles is new; the end-to-end safety cost of PQ in a full SCMS remains open as far as this abstract shows.
