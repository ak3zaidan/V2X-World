# Policy-Governed Post-Quantum Migration for Legacy Microservices: An Ephemeral Sidecar Pattern with Crypto-Agility, Performance Budgets, and Rollback Safety
Shuaib Ahmed · 2026 International Conference on Intelligent and Sustainable Electronics & Computing Technologies (INSECT) 2026 · https://doi.org/10.1109/insect68872.2026.11663812 · relevance: low · basis: abstract

**Problem:** Quantum computing has a potential threat to designed microservice architectures of the past due to classical cryptographic algorithms. It is not easy to migrate these systems to post-quantum cryptography (PQC) because of the performance overheads, complexity in its operation and compatibility with existing services.
**Approach:** The paper suggests a Policy-Governed Post-Quantum Migration Framework (PG-PQM) which will allow the introduction of PQC in legacy microservices without any major changes to the application level. The architecture proposed is also designed to incorporate adaptive security scoring, automatic rollback and ephemeral key management to offer reliability in the process of migration.
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** Findings indicate that the suggested framework can cut down on the migration overheads by approximately 28% relative to the fixed PQC deployment strategies, and yet minimizes the latency overheads to less than 12 percent and enhances throughput by 18% under dynamic workload settings. Moreover, the automated roll back system effectively reduced more than 95% of the performance anomalies in terms of transitioning ...
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
