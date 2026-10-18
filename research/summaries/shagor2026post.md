# Post-Quantum and Trust-Aware Authentication for Permissioned Vehicular Blockchains: Session-Amortized V2X Security with Historical Key Continuity
Md Shahanur Islam Shagor · Research Square 2026 · https://doi.org/10.21203/rs.3.rs-11073259/v1 · relevance: high · basis: full text

**Problem:** Signing every V2X message with PQ signatures is costly over long vehicle lifetimes.
**Approach:** ML-DSA-44 identities and signed key transitions, ML-KEM-512 sessions, session-bound MACs for routine messages, bounded trust state with quarantine, on a permissioned vehicular blockchain.
**Tools and scale:** Discrete-event simulation with FIPS 203/204 sizes and stated service-time assumptions; up to 500 vehicles; not road measurements.
**Results:** Routine authentication overhead 2,452 bytes (ML-DSA) to 74 bytes; median latency 20.8 ms vs 39.6 ms at 500 vehicles; 96.2-98.1% malicious message rejection with benign quarantine under 1.6%.
**Limitations and future work (as stated by the authors):**
- Ledger tamper evidence only within permissioned governance; malicious coalition can rewrite policy.
- Trust biased by colluding observations or poor thresholds.
- Future work: automotive hardware, broadcast and cross-domain roaming, hardware-backed key state, validation with real traces and safety-specific misbehaviour models.
**Overlap with our engine:** partial — PQ overhead amortisation and trust-based admission relate to direction 2 and MBD, but unicast session MACs differ from 1609.2 broadcast.
**Novelty implications:** Session-amortised PQ for V2X is proposed; broadcast safety messaging with SCMS certificates is still the harder open case.
