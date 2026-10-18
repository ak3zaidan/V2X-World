# Comment on “An advanced blockchain-based mutual authentication technique for the Internet of vehicles environment”: cryptanalysis and security vulnerabilities
Yashar Salami · The Journal of Supercomputing 2026 · https://doi.org/10.1007/s11227-026-08570-x · relevance: low · basis: abstract

**Problem:** The Internet of Vehicles (IoV), in which vehicles exchange safety–critical information over open wireless channels, is characterized by large-scale, distributed operation, and stringent real-time constraints, requiring efficient and reliable communication mechanisms. Recently, Suyel et al. proposed a blockchain-based mutual authentication and key agreement protocol for IoV ...
**Approach:** Recently, Suyel et al. proposed a blockchain-based mutual authentication and key agreement protocol for IoV environments (The Journal of Supercomputing, 2025, https://doi.org/10.1007/s11227-025-07934-z ), claiming resistance against various attacks under the Dolev–Yao adversarial model. In this paper, we revisit the security of this protocol and demonstrate that its V2V authentication mechanism suffers from a ...
**Tools and scale:** Vehicular setting (VANET/IoV/V2X). Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** Our results reveal that the protocol fails to ensure reliable vehicle authentication and cannot securely attribute the established session key to the claimed peer identity.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial: a V2X authentication/privacy scheme; our engine implements the standard SCMS/IEEE 1609.2 path rather than this protocol, and the paper reports no closed-loop traffic, radio or revocation simulation.
**Novelty implications:** Rules out nothing specific in our candidate directions. It shows that lightweight V2X authentication protocols are widely proposed; we should not claim a new authentication scheme. Open: evaluation of such schemes in a closed-loop city-scale simulation with radio and safety ground truth.
