# Provably Secure Drone-Assisted Authentication Protocol for Emergency Rescue in VANETs
Qing-Yun Xie, Nan Fang · Symmetry 2026 · https://doi.org/10.3390/sym18071204 · relevance: low · basis: abstract

**Problem:** After an earthquake or major traffic accident, emergency rescue vehicles may be unable to access the accident site or communicate with the survey drones at the scene. In such cases, an emergency rescue vehicle can select any nearby drone as a communication relay to establish a connection with survey drones at the accident site, thereby forming a drone-assisted Vehicular Ad Hoc ...
**Approach:** Therefore, this paper proposes a lightweight drone-assisted emergency rescue authentication protocol based on Physical Unclonable Functions (PUF) and the Chebyshev chaotic map. The proposed scheme not only resists side-channel attacks and device capture attacks but also addresses the single point of failure problem of relay drones.
**Tools and scale:** Vehicular setting (VANET/IoV/V2X). Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** Both formal and informal security analyses verify its security, and comparative experiments show that it has lower computational and communication overhead, making it more suitable for emergency rescue application scenarios.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial: a V2X authentication/privacy scheme; our engine implements the standard SCMS/IEEE 1609.2 path rather than this protocol, and the paper reports no closed-loop traffic, radio or revocation simulation.
**Novelty implications:** Rules out nothing specific in our candidate directions. It shows that lightweight V2X authentication protocols are widely proposed; we should not claim a new authentication scheme. Open: evaluation of such schemes in a closed-loop city-scale simulation with radio and safety ground truth.
