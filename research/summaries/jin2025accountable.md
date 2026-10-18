# Accountable, Scalable and DoS-resilient Secure Vehicular Communication
Hongyu Jin, Panos Papadimitratos · Computers & Security (2025): 104469 2025 · https://doi.org/10.1016/j.cose.2025.104469 · relevance: high · basis: full text

**Problem:** Forged CAMs/DENMs force validation and exhaust resources (clogging DoS).
**Approach:** Message verification facilitators to prioritise likely-valid messages and verify multiple messages per signature; cooperative verification with probabilistic checking and cross-checking.
**Tools and scale:** Simulation with realistic DENM dissemination model.
**Results:** Average beacon verification latency 50 ms with under 1% expiration under DoS; 80% DENM acceptance with under 200 ms latency vs 50-100% expiration in baseline.
**Limitations and future work (as stated by the authors):**
- None stated.
**Overlap with our engine:** strong — Verification-load DoS and prioritisation policy, a core part of direction 2 and NR attacks with verification load.
**Novelty implications:** Cannot claim first DoS-resilient verification scheduling; open: closed-loop safety impact and PQ signatures.
