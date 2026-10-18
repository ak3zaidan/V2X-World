# AC-DC: Adaptive Ensemble Classification for Network Traffic Identification
Xi Jiang, Shinan Liu, Saloua Naama, F. Bronzino, Paul Schmitt, N. Feamster · arXiv.org 2023 · https://arxiv.org/abs/2302.11718 · relevance: low · basis: abstract

**Problem:** Accurate and efficient network traffic classification is important for many network management tasks, from traffic prioritization to anomaly detection.
**Approach:** Although classifiers using pre-computed flow statistics (e.g., packet sizes, inter-arrival times) can be efficient, they may experience lower accuracy than techniques based on raw traffic, including packet captures. Past work on representation learning-based classifiers applied to network traffic captures has shown to be more accurate, but slower and requiring considerable additional memory resources, due to the substantial costs in feature preprocessing.
**Tools and scale:** Not stated in the abstract.
**Results:** Our evaluation shows that AC-DC improves classification performance by more than 100% compared to classifiers that rely on flow statistics alone; compared to the state-of-the-art packet-capture classifiers, AC-DC achieves comparable performance (less than 12.3% lower in F1-Score), but processes traffic over 150x faster.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — adjacent background; no V2X security, PKI or simulator component.
**Novelty implications:** Nothing in our candidate directions is ruled out; useful only as background.
