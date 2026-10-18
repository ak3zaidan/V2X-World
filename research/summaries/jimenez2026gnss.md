# GNSS Spoofing Threat for V2X communications
Adolfo P. Jimenez, Juan Arquero-Gallego, Mario P. Luna et al. · arXiv 2026 · https://arxiv.org/abs/2606.20215 · relevance: medium · basis: abstract

**Problem:** Global Navigation Satellite Systems (GNSS) constitute a core technology for delivering crucial positioning, navigation, and timing (PNT) services in the Vehicle-to-Everything (V2X) domain, where they are indispensable for generating Cooperative Awareness Messages (CAM) that uphold network reliability and vehicular safety.
**Approach:** This work presents a methodology for conducting physical spoofing with inexpensive Software Defined Radio (SDR), describing a coordinate generation pipeline that employs Haversine-based distance calculations, temporal discretization to emulate constant velocity, and linear interpolation to produce high-fidelity GPS baseband signals. The proposed attack is experimentally validated on real Commsignia OnBoard Unit (OBU) and RoadSide Unit (RSU) devices using a HackRF One across three scenarios that emulate synthetic trajectories at steady speeds of 90 km/h, 145 km/h, and 200 km/h.
**Tools and scale:** Not stated in the abstract.
**Results:** The most significant contribution of this paper is the demonstration that V2X communications are not secured, as they are susceptible to GNSS spoofing attacks, which cause service degradation without being detected.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses misbehaviour/attack detection (direction 1 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
