# Survey of Detection Techniques for GPS Spoofing in Connected Vehicles: Taxonomy, Evaluation, and Future Research Directions
Ying He · Informatica 2025 · https://doi.org/10.31449/inf.v49i27.9504 · relevance: medium · basis: abstract

**Problem:** The Global Positioning System (GPS) plays a pivotal role in modern vehicular systems by providing accurate navigation and timing services.
**Approach:** Future directions are proposed to address these limitations and improve the reliability and scalability of GPS spoofing detection systems in real-world connected vehicle applications.
**Tools and scale:** For each category, we analyze the methods’ underlying principles, datasets used (e.g., GNSS-SDR, USRP, simulation-based testbeds), and performance metrics including accuracy, precision, recall, and F1 score.
**Results:** For each category, we analyze the methods’ underlying principles, datasets used (e.g., GNSS-SDR, USRP, simulation-based testbeds), and performance metrics including accuracy, precision, recall, and F1 score. The comparative evaluation highlights that hybrid methods and sensor fusion approaches generally offer higher robustness, while ML-based methods achieve high accuracy but require extensive training data.
**Limitations and future work (as stated by the authors):**
- "The paper also identifies key challenges, such as a lack of benchmarking standards and performance variability across dynamic environments."
- "Future directions are proposed to address these limitations and improve the reliability and scalability of GPS spoofing detection systems in real-world connected vehicle applications."
**Overlap with our engine:** partial — addresses misbehaviour/attack detection (direction 1 area); datasets/twins/surrogates (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
