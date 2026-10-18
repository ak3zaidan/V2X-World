# Adaptive trust and privacy management framework for vehicular networks
T. Pham, C. Yeo · Vehicular Communications 2018 · https://doi.org/10.1016/j.vehcom.2018.04.006 · relevance: medium · basis: abstract

**Problem:** By allowing vehicles to communicate on the roads, vehicular network is becoming a potential solution to improve the traffic safety. Both trust management and privacy protection play critical roles in vehicular network but there needs to be a trade-off between them.
**Approach:** In this paper, we propose a secure and flexible framework for vehicles to manage both trust and privacy. First, we design ALRS – a secure linkability scheme to enable vehicles to recognize either identities or trust levels of other vehicles despite them updating the pseudonyms to protect privacy. Second, we design ATMS – a context-aware trust management scheme that allows nodes to evaluate the trustworthiness of received events by considering the entity reputations of the senders.
**Tools and scale:** Not stated in the abstract.
**Results:** Simulation results demonstrate that our framework helps nodes to simultaneously make accurate decisions towards the data and preserve their privacy in a flexible way.
**Limitations and future work (as stated by the authors):**
- To overcome the challenge, we utilize the linkability information in ALRS and design a decision tree that estimates the entity trust adaptively to the available linkability information.
**Overlap with our engine:** partial — Trust/reputation management for detecting malicious senders; an alternative to the SCMS misbehaviour-report -> MA -> CRL pipeline our engine models, with no linkage-based revocation or time-to-neutralise analysis.
**Novelty implications:** Does not rule out any candidate direction; it does not model SCMS certificates, linkage-based revocation, per-message joint ground truth or counterfactual reruns.
