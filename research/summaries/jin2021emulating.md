# Towards Emulating Internet-of-Vehicles on a Single Machine
Wangkai Jin, Xiaoxing Ming, Zilin Song, Zeyu Xiong, Xiangjun Peng · 13th International Conference on Automotive User Interfaces and Interactive Vehicular Applications 2021 · https://doi.org/10.1145/3473682.3480275 · relevance: low · basis: abstract

**Problem:** Techniques for Human-Vehicle Interaction can be easily bounded by the limited amount of computational resources within the vehicles.
**Approach:** Therefore, outsourcing the computations from vehicles into a powerful, centralized server becomes the prime method in practice. Such formalization between edge vehicles and centralized servers is denoted as the Internet-of-Vehicles, which forms multiple vehicles as a distributed system.
**Tools and scale:** Vehicular setting. The available text does not describe a V2X simulator, radio stack, traffic model or SCMS-style PKI in enough detail to report; scale not stated.
**Results:** The key idea of our work is two-folded: (1) we provide an automatic extractor to retrieve the key patterns from Human-Vehicle Interaction workloads, so that our platform can facilitate with the needs of different scenarios/techniques; and (2) we leverage the configurable networking connections to provide abstractions regarding the interactions between edge vehicles and centralized servers, so that we can enable various types of emulations (e.g.
**Limitations and future work (as stated by the authors):**
- Our results reveal the impacts of different implementations under the Internet-of-Vehicles setting, and our future works would focus on enhancing the characteristics of our prototype and scaling our study into a broader range of applications in Human-Vehicle Interactions.
**Overlap with our engine:** none: adjacent topic in a vehicular context. Low relevance background; it does not model the joint traffic/radio/SCMS loop, closed-loop revocation or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the integrated V2X case (SCMS scale, channel load, closed-loop safety impact) is not addressed.
