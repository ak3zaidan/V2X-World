# Accelerating IoV Intrusion Detection: Benchmarking GPU-Accelerated vs CPU-Based ML Libraries
Furkan Çolhak, Hasan Coşkun, Tsafac Nkombong Regine Cyrille et al. · arXiv 2025 · https://arxiv.org/abs/2504.01905 · relevance: medium · basis: abstract

**Problem:** The Internet of Vehicles (IoV) may face challenging cybersecurity attacks that may require sophisticated intrusion detection systems, necessitating a rapid development and response system.
**Approach:** This research investigates the performance advantages of GPU-accelerated libraries (cuML) compared to traditional CPU-based implementations (scikit-learn), focusing on the speed and efficiency required for machine learning models used in IoV threat detection environments.
**Tools and scale:** Not stated in the abstract.
**Results:** Our findings demonstrate that GPU-accelerated implementations dramatically improved computational efficiency, with training times reduced by a factor of up to 159 and prediction speeds accelerated by up to 95 times compared to traditional CPU processing, all while preserving detection accuracy.
**Limitations and future work (as stated by the authors):**
- "The Internet of Vehicles (IoV) may face challenging cybersecurity attacks that may require sophisticated intrusion detection systems, necessitating a rapid development and response system."
**Overlap with our engine:** partial — addresses misbehaviour/attack detection (direction 1 area); datasets/twins/surrogates (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
