# A Quantum-Resistant Key Management Scheme Using Blockchain in C-V2X
Yujing Gong, Bin-Jie Hu · IEEE transactions on intelligent transportation systems (Print) 2024 · https://doi.org/10.1109/tits.2024.3421381 · relevance: high · basis: abstract

**Problem:** C-V2X key management must resist quantum attacks across uplink/downlink, sidelink and backhaul.
**Approach:** Blockchain at base stations stores vehicle public keys; lattice-based group-key distribution for beacons and session keys for P2P; region-based key updates; revocation on misbehaviour detection.
**Tools and scale:** Analytical/informal security analysis; comparative cost analysis (computation, communication, storage, energy). No simulator stated.
**Results:** Claims favourable comparative costs; no figures in abstract.
**Limitations and future work (as stated by the authors):**
- Security against attacks analysed only informally
**Overlap with our engine:** partial — PQ key management with misbehaviour-triggered revocation, but blockchain-based and not SCMS/1609.2; no traffic or radio simulation.
**Novelty implications:** PQ revocation schemes exist on paper; closed-loop simulated revocation-to-safety evaluation remains open.
