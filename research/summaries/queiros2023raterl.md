# RateRL: A Framework for Developing RL-Based Rate Adaptation Algorithms in ns-3
Ruben Queiros, Luis Ferreira, Helder Fontes, Rui Campos · arXiv 2023 · https://arxiv.org/abs/2310.05772 · relevance: low · basis: abstract

**Problem:** The increasing complexity of recent Wi-Fi amendments is making the use of traditional algorithms and heuristics unfeasible to address the Rate Adaptation (RA) problem.
**Approach:** This is due to the large combination of configuration parameters along with the high variability of the wireless channel. Recently, several works have proposed the usage of Reinforcement Learning (RL) techniques to address the problem.
**Tools and scale:** Non-vehicular or unspecified setting. The available text does not describe a V2X simulator, radio stack, traffic model or SCMS-style PKI in enough detail to report; scale not stated.
**Results:** This paper proposes a framework, named RateRL, that integrates state of the art libraries with the well-known Network Simulator 3 (ns-3) to enable the implementation and evaluation of RL-based RA algorithms.
**Limitations and future work (as stated by the authors):**
- Also, the lack of standard frameworks enabling the reproducibility of results and the limited availability of source code, makes the fair comparison with state of the art approaches a challenge.
**Overlap with our engine:** none: simulation/digital twins outside V2X. Low relevance background for the simulator itself; it does not model the joint traffic/radio/SCMS loop, closed-loop revocation or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the integrated V2X case (SCMS scale, channel load, closed-loop safety impact) is not addressed.
