# BERT-Based Intrusion Detection System for RF Jamming Attacks in Vehicular Network
Nujitha Wickramasurendra · Brock University Digital Repository (Brock University) 2025 · https://hdl.handle.net/10464/19649 · relevance: medium · basis: abstract

**Problem:** As vehicular networks continue to evolve toward increased connectivity and autonomy, they become more vulnerable to cybersecurity threats, particularly Radio Frequency (RF) jamming attacks that can severely disrupt communication systems.
**Approach:** This thesis presents a comprehensive study on the application of transformer-based models for RF jamming detection in intelligent transportation systems.
**Tools and scale:** Not stated in the abstract.
**Results:** Our results show that transformer models significantly outperform the baselines in terms of classification accuracy and robustness under mobility-induced signal variation. At high speed (25 m/s), BERT achieved the best performance with an accuracy of 96.25%, while at low speed (15 m/s)—a more challenging scenario due to reduced temporal dynamics—DistilBERT led with an accuracy of 91.00%.
**Limitations and future work (as stated by the authors):**
- "At high speed (25 m/s), BERT achieved the best performance with an accuracy of 96.25%, while at low speed (15 m/s)—a more challenging scenario due to reduced temporal dynamics—DistilBERT led with an accuracy of 91.00%."
- "However, distinguishing between smart and constant jamming attacks remained challenging, especially at lower speeds."
- "Future work will explore lightweight fine-tuning techniques and real-world deployment scenarios."
**Overlap with our engine:** partial — addresses misbehaviour/attack detection (direction 1 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
