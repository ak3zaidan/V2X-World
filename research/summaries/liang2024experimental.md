# Experimental evaluation of a machine learning approach to improve the reproducibility of network simulations
Luke Liang, Hieu Phan, Philippe J Giabbanelli · SIMULATION 2024 · https://doi.org/10.1177/00375497241229753 · relevance: low · basis: abstract

**Problem:** A stochastic network simulation is verified when its distribution of outputs is aligned with the ground truth, while tolerating deviations due to variability in real-world measurements and the randomness of a stochastic simulation.
**Approach:** However, comparing distributions may yield false positives, as erroneous simulations may have the expected distribution yet present aberrations in low-level patterns. For instance, the number of sick individuals may present the right trend over time, but the wrong individuals were infected.
**Tools and scale:** Vehicular setting. The available text does not describe a V2X simulator, radio stack, traffic model or SCMS-style PKI in enough detail to report; scale not stated.
**Results:** In this paper, we examine whether our approach can be utilized with very small data sets (e.g., 5–10 images), as provided in publications.
**Limitations and future work (as stated by the authors):**
- Although a good accuracy is obtained in detecting several forms of errors, this paper is only a first step in the use of this technique for verification; hence, future works should assess the applicability of our approach to other types of network simulations.
**Overlap with our engine:** none: misbehaviour/intrusion detection in a vehicular context. Low relevance background for direction 1 (closed-loop misbehaviour to revocation); it does not model the joint traffic/radio/SCMS loop, closed-loop revocation or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the integrated V2X case (SCMS scale, channel load, closed-loop safety impact) is not addressed.
