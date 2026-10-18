# A Hybrid Post Quantum Cryptography and Quantum Machine Learning Framework for Secure IoT Anomaly Detection
Lokesh B S, Narasimha Kaulgud · venue unknown 2025 · https://doi.org/10.21203/rs.3.rs-7538381/v1 · relevance: low · basis: abstract

**Problem:** Experimental results show that qSVM consistently outperforms classical SVM and XGBoost by 4--7% in accuracy, precision, recall, and F1-score, achieving up to 97.6% accuracy with encryption overhead limited to 5--7%.
**Approach:** This work introduces a hybrid framework that integrates post-quantum cryptography (PQC), quantum key distribution (QKD), and quantum machine learning (QML) to deliver quantum-resilient anomaly detection.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** A novel PQC-IoT dataset is generated using real-time Raspberry Pi sensors, enriched with 30+ features spanning 12 traffic classes (9 attacks, 3 benign), and incorporating PQC-specific metadata such as encryption latency and ciphertext size. Security is enforced through Kyber-512 encryption and a BB84-based QKD channel, while anomaly detection is performed using a quantum support vector machine (qSVM).
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — post-quantum/hybrid authentication scheme; the engine implements PQ and hybrid signatures, but the abstract states no SCMS-scale or traffic-coupled evaluation.
**Novelty implications:** We cannot claim PQ authentication for vehicles is new; the end-to-end safety cost of PQ in a full SCMS remains open as far as this abstract shows.
