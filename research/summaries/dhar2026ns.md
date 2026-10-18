# ns-3 Based High-Fidelity Simulation of an Internet of Battlefield Things Network for IFF Reliability Study and Digital Twin Candidate Assessment
Anurag Dhar, Rachit Ahluwalia, Santosh Kumar · unknown venue 2026 · https://doi.org/10.21203/rs.3.rs-9150618/v1 · relevance: low · basis: abstract

**Problem:** Abstract We report an ns-3 simulation of an IoBT network built to quantify IFF reliability under terrain-heterogeneous LoRaWAN propagation.
**Approach:** The testbed models a 10 × 10 km battlespace with 200 Class A end-devices, hexagonal gateway grids at five inter-gateway separations (200–7000 m), and nine terrain classes whose path-loss exponents (n = 1.8 to 4.4) come from published LoRa field measurements. A custom event-level logger captures every transmission and gateway reception at millisecond resolution; the consolidated dataset covers 3.26 × 10⁵ communication events across 45 independent ...
**Tools and scale:** Non-vehicular or unspecified setting. The available text does not describe a V2X simulator, radio stack, traffic model or SCMS-style PKI in enough detail to report; scale not stated.
**Results:** The testbed models a 10 × 10 km battlespace with 200 Class A end-devices, hexagonal gateway grids at five inter-gateway separations (200–7000 m), and nine terrain classes whose path-loss exponents (n = 1.8 to 4.4) come from published LoRa field measurements. A custom event-level logger captures every transmission and gateway reception at millisecond resolution; the consolidated dataset covers 3.26 × 10⁵ communication events across 45 independent ...
**Limitations and future work (as stated by the authors):**
- None stated in the available text.
**Overlap with our engine:** none: simulation/digital twins outside V2X. Low relevance background for the simulator itself; it does not model the joint traffic/radio/SCMS loop, closed-loop revocation or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the integrated V2X case (SCMS scale, channel load, closed-loop safety impact) is not addressed.
