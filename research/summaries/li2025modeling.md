# Modeling the Interdependent Coupling of Safety and Security for Connected and Automated Vehicles: A Copula-Based Integrated Risk Analysis Approach
Xingyu Li, Qi Liu, Yufeng Li · arXiv 2025 · https://arxiv.org/abs/2512.14748 · relevance: low · basis: abstract

**Problem:** Safety and security are critical to the reliable operation of connected and automated vehicles (CAVs).
**Approach:** While existing research has identified correlations between the two domains, a theoretical framework to analyze their interaction mechanisms and guide co-design remains lacking. To address this gap, this paper proposes a copula-based joint safety-security analysis method to quantify their coupling effects.
**Tools and scale:** Vehicular setting. The available text does not describe a V2X simulator, radio stack, traffic model or SCMS-style PKI in enough detail to report; scale not stated.
**Results:** First, we formulate time-varying cyberattacks using dynamic risk functions derived from survival analysis, while modeling random hardware failures with the Weibull distribution, as per the automotive industry standard ISO 26262. Furthermore, we provide formal theoretical analysis of the dependence structure in the safety-security coupling, yielding three key insights: (1) a monotonic relationship between joint failure probability and dependence ...
**Limitations and future work (as stated by the authors):**
- While existing research has identified correlations between the two domains, a theoretical framework to analyze their interaction mechanisms and guide co-design remains lacking.
**Overlap with our engine:** none: simulation/digital twins in a vehicular context. Low relevance background for the simulator itself; it does not model the joint traffic/radio/SCMS loop, closed-loop revocation or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the integrated V2X case (SCMS scale, channel load, closed-loop safety impact) is not addressed.
