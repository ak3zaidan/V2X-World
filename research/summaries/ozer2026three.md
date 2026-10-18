# Three Axes of Quantum Risk: A Unified Observability Model for PQC, QKD, and Crypto-Agility
Aleaddin Ozer, Murat Aydos · venue not stated 2026 · https://doi.org/10.21203/rs.3.rs-10267693/v1 · relevance: low · basis: abstract

**Problem:** The post-quantum migration has moved from research to operations. NIST has finalised FIPS 203/204/205, the NSA’s CNSA 2.0 timetable runs to 2033, and Quantum Key Distribution is being deployed on high-assurance links under ETSI GS QKD 014.
**Approach:** We treat quantum-risk posture as a three-axis observation problem. Each asset is graded on its algorithmic resistance (A), the channel through which its keys are delivered (C), and how quickly its primitive can be replaced (G), and the three are folded into a deadline-adjusted score 𝑞(asset, 𝑡) whose agility weight shrinks as the operator’s deadline approaches.
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** To make the model observable, we extend the open crypto_inventory_event v1 schema with two new fields, build a reference implementation (Sezar) covering eBPF-based TLS observation, an ETSI GS QKD 014 collector with a working Key-Management -Entity emulator, and a static crypto- agility scanner, and evaluate the result against three reproducible studies: a Tranco-top-1k TLS handshake survey, a controlled ...
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
