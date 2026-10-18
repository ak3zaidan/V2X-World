# An Enhanced and Explainable Hybrid Ensemble Intrusion Detection System for Connected Vehicles
Xuemei Li, Rui Zhu, Huirong Fu · Journal of Information Security 2025 · https://doi.org/10.4236/jis.2025.163020 · relevance: medium · basis: abstract

**Problem:** Intelligent vehicles require strong cybersecurity.
**Approach:** This paper proposes a Hybrid Ensemble MLP IDS (HE-MLP IDS) combining auto-encoder-based feature extraction, ensemble neural networks, and weighted voting.
**Tools and scale:** Not stated in the abstract.
**Results:** Evaluated with varying architectures and explained using XAI, the model achieves 0.99 accuracy, recall, and F1-score, with a 7.8e−4 false alarm rate. It outperforms benchmarks in memory size and prediction speed, while effectively identifying attack-relevant features from CAN and network logs.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses misbehaviour/attack detection (direction 1 area); datasets/twins/surrogates (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
