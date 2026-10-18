# Beyond Line of Sight: Hybrid Validation of V2X Collective Perception in Complex Scenarios
Markos Antonopoulos, Anastasia Bolovinou, Bill Roungas, Elena Daskalaki, Angelos Amditis · arXiv 2026 · https://arxiv.org/abs/2607.00874 · relevance: low · basis: abstract

**Problem:** This paper introduces a probabilistic framework and hybrid validation methodology for V2X-enabled Collective Perception (CP) in complex traffic scenarios.
**Approach:** The proposed Bayesian fusion algorithm extends the perceptual horizon of connected and autonomous vehicles by integrating heterogeneous sensor observations from multiple agents into a shared probabilistic occupancy grid. Each cell of this grid encapsulates both occupancy likelihood and uncertainty, enabling explainable and trustworthy situational awareness beyond the ego vehicle's field of view.
**Tools and scale:** Vehicular setting. The available text does not describe a V2X simulator, radio stack, traffic model or SCMS-style PKI in enough detail to report; scale not stated.
**Results:** Experimental results in a roundabout scenario demonstrate a 260 percent increase in field-of-view coverage and a rise in occupied-cell recall from 0.82 (ego-only) to 0.94 (six-agent CP) under nominal localization conditions.
**Limitations and future work (as stated by the authors):**
- None stated in the available text.
**Overlap with our engine:** none: simulation/digital twins in a vehicular context. Low relevance background for the simulator itself; it does not model the joint traffic/radio/SCMS loop, closed-loop revocation or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the integrated V2X case (SCMS scale, channel load, closed-loop safety impact) is not addressed.
