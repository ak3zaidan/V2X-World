# Systematic Error Correction of SUMO Traffic Simulator's HBEFA Vehicle Emission Model
Balázs Varga, Zoran Lulić, Tamás Tettamanti · 2024 IEEE International Conference on Electrical Systems for Aircraft, Railway, Ship Propulsion and Road Vehicles &amp;amp; International Transportation Electrification Conference (ESARS-ITEC) 2024 · https://doi.org/10.1109/esars-itec60450.2024.10819857 · relevance: low · basis: abstract

**Problem:** This paper analyses the HBEFA-based instantaneous vehicle emissions model implemented in the SUMO microscopic traffic simulation.
**Approach:** We identify significant discrepancies between real vehicle emissions measured with portable emission measurement systems and the modeled emissions. Corrections to the model are proposed to account for road gradients, transients, and engine management functions such as fuel cut-off, start-stop systems, and hybrid engine management.
**Tools and scale:** Vehicular setting. The available text does not describe a V2X simulator, radio stack, traffic model or SCMS-style PKI in enough detail to report; scale not stated.
**Results:** The proposed corrections are validated against real measurements, resulting in an accuracy gain of over 50% at the trajectory level.
**Limitations and future work (as stated by the authors):**
- None stated in the available text.
**Overlap with our engine:** none: threshold cryptography in a vehicular context. Low relevance background for threshold authorities; it does not model the joint traffic/radio/SCMS loop, closed-loop revocation or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the integrated V2X case (SCMS scale, channel load, closed-loop safety impact) is not addressed.
