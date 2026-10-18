# HybridTrust: on-device federated learning with crypto-agile security for legacy and quantum-safe medical devices
Umar Hayat Khan, Rahim Khan, S. Chelloug, Fahad Alturise et al. · Scientific Reports 2026 · https://doi.org/10.1038/s41598-026-52891-z · relevance: low · basis: abstract

**Problem:** However, there exists an important interoperability challenge posed by the need for collaboration within the domain of edge intelligence in critical life situations like ICU monitoring.
**Approach:** Here, existing ECC-based devices alongside newly developed quantum-resistant devices will be operating within the same healthcare ecosystem. This paper presents HybridTrust, an on-device federated TinyML framework designed to enable secure collaboration among legacy, post-quantum, and hybrid IoMT devices during the post-quantum cryptography migration period.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** The framework integrates a dynamic crypto-negotiation protocol that selects the strongest mutually supported scheme among ECC, ML-KEM-512, and hybrid ECC–ML-KEM operation; a compact 931-byte hybrid certificate format suitable for ESP32-class microcontrollers; a regulatory compliance scorecard mapping cryptographic choices to HIPAA, GDPR, FDA, and NIST expectations; and a tree-concatenation-based model fusion strategy for maintaining clinical utility under non-IID hospital data distributions. The framework achieves an average anomaly detection accuracy of 90.5% while introducing only 0.09–0.43 ms quantum-safe cryptographic overhead per operation.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — post-quantum/hybrid authentication scheme; the engine implements PQ and hybrid signatures, but the abstract states no SCMS-scale or traffic-coupled evaluation.
**Novelty implications:** We cannot claim PQ authentication for vehicles is new; the end-to-end safety cost of PQ in a full SCMS remains open as far as this abstract shows.
