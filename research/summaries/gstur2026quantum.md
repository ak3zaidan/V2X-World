# Quantum-Resistant Crypto-Agile Inline Authentication and Encryption Framework for IEC 61850 Digital Substations
Moritz Gstür, Mohammed Ramadan, Veit Hagenmeyer · Proceedings of the 2026 ACM Sustainability Week 2026 · https://doi.org/10.1145/3765611.3815134 · relevance: low · basis: abstract

**Problem:** Smart electricity grids increasingly rely on communication between distributed subsystems, leading to an increased attack surface. Standards for smart grid communication security, such as IEC 62351, do not sufficiently cover the developments of quantum computing.
**Approach:** Standards for smart grid communication security, such as IEC 62351, do not sufficiently cover the developments of quantum computing. To bridge this gap, we propose a novel quantum-resistant crypto-agile authentication and encryption framework.
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** The framework is tailored to the strict time constraints of low-latency protocols deployed in IEC 61850 digital substations. The results show that the framework is able to secure low-latency digital substation communication, as authenticated and encrypted frames achieve a transfer time below 3 ms.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
