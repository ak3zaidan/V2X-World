# A distributed prevention scheme from malicious nodes in VANETs' routing protocols
Tarek Bouali, Hichem Sedjelmaci, Sidi‐Mohammed Senouci · IEEE Wireless Communications and Networking Conference 2016 · https://doi.org/10.1109/wcnc.2016.7564928 · relevance: medium · basis: abstract

**Problem:** Vehicular environments are vulnerable to attacks because of the continuous interactions between vehicles despite authentication techniques deployed by communication standards.
**Approach:** In fact, an authenticated node with a certificate could initiate an attack while complying with implemented protocols if it has malicious intentions and benefit from this always on connection to threaten the network accuracy. Several mechanisms to counter these attacks were proposed but none of them is able to anticipate the behavior of nodes.
**Tools and scale:** Not stated in the abstract.
**Results:** Results, given by the implementation of the proposed mechanism over an intersection-based routing protocol using ns3 simulator, prove its accuracy regarding the detection rate and a good impact on packets delivery ratio and end-to-end delay.
**Limitations and future work (as stated by the authors):**
- We use Kalman filter to predict the future behavior of vehicles and classify them into three categories (white, gray and black) based on their expected trustworthiness.
**Overlap with our engine:** partial — same area (V2X/vehicular security or networking) with a narrower, mechanism-level focus.
**Novelty implications:** Does not remove any of our claims beyond its specific mechanism; we cannot claim to be first on that mechanism. The joint closed-loop traffic/radio/PKI evaluation with counterfactual ground truth remains open as far as this abstract shows.
