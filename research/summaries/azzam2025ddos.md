# DDoS Detection in VANETs Using a Multilayer Perceptron and Simple Blockchain-Based Trust Management for False Positive Minimization
Ibrahim Abdullah Azzam, Galih Nugraha Nurkahfi, Hendrawan, Nana Rachmana Syambas · International Conference on Telecommunication Systems, Services, and Applications 2025 · https://doi.org/10.1109/tssa68467.2025.11303848 · relevance: medium · basis: abstract

**Problem:** Vehicular Ad-hoc Networks (VANETs) are critical for safety messaging in Intelligent Transportation Systems yet are highly vulnerable to Distributed Denial-of-Service (DDoS) attacks that disrupt safety-critical communications.
**Approach:** This paper proposes a low-latency detection framework that combines a Multilayer Perceptron (MLP) classifier with a lightweight blockchain-backed reputation scoring mechanism. The framework augments traditional Quality of Service (QoS) features with historical trust values derived from direct and indirect evidence, which are recorded on a ledger.
**Tools and scale:** Mentioned in abstract: blockchain
**Results:** Integrating reputation features consistently improves detection performance: macro F1-scores rose from 0.62-0.70 in the baseline (QoS-only) to 0.88-0.95 in the proposed configuration. Precision and recall gains were most pronounced for classes prone to misclassification in the baseline, significantly reducing false positives in benign high-load scenarios.
**Limitations and future work (as stated by the authors):**
- Future work will address this gap by scaling the blockchain node deployment, evaluating the system's resilience to more advanced adversarial conditions such as Sybil attacks, and implementing real-time cross-domain testing to enhance generalization.
**Overlap with our engine:** partial — same area (V2X/vehicular security or networking) with a narrower, mechanism-level focus.
**Novelty implications:** Does not remove any of our claims beyond its specific mechanism; we cannot claim to be first on that mechanism. The joint closed-loop traffic/radio/PKI evaluation with counterfactual ground truth remains open as far as this abstract shows.
