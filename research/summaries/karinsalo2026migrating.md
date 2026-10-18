# Migrating Time and Security-Critical PKIs to Post-Quantum Cryptography: SWIM and C-ITS
A. Karinsalo, Sara Nikula, Sami Lehtonen · International Conference on Cyber Warfare and Security 2026 · https://doi.org/10.34190/iccws.21.1.4479 · relevance: high · basis: full text

**Problem:** Time- and security-critical PKIs (SWIM, C-ITS) must migrate to PQC under latency, bandwidth, interoperability and regulation constraints.
**Approach:** Constraint-driven comparative analysis synthesising published benchmarks, standards and protocol specs on certificate size, verification latency and session establishment.
**Tools and scale:** Analytical synthesis, no new implementation.
**Results:** Direct replacement of classical algorithms by PQC is generally infeasible in critical PKIs without structural changes to certificate handling and protocols.
**Limitations and future work (as stated by the authors):**
- No new measurements; synthesis only.
- Future work: system-level validation of PQC-enabled PKIs under realistic traffic conditions.
- Future work: standards for multi-algorithm operation and crypto agility.
**Overlap with our engine:** partial — PQ migration of the C-ITS PKI, exactly direction 2's subject, but analytical.
**Novelty implications:** Analytical infeasibility claims exist; the system-level validation under realistic traffic they call for is open and is what our engine can provide.
