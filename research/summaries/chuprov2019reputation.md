# Reputation and Trust Approach for Security and Safety Assurance in Intersection Management System
Sergey Chuprov, Ilya Viksnin, Iuliia Kim, Egor Marinenkov et al. · Energies 2019 · https://doi.org/10.3390/en12234527 · relevance: medium · basis: abstract

**Problem:** Crossroads are the main traffic jam generators in densely populated cities. Unmanned vehicles and intelligent transportation systems can significantly reduce congestion and improve road safety by eliminating the main cause of traffic accidents—the human factor.
**Approach:** This paper addresses the contextual data integrity problem, when an unmanned autonomous vehicle transmits incorrect data due to technical problems, or malicious attacks. We propose an approach based on trust and reputation that allows detecting vehicles transmitting bogus data.
**Tools and scale:** Car-Hacking/CAN dataset (mentioned in abstract)
**Results:** To verify the feasibility of the approach on practice, we conducted both software and physical simulations using the model of intersection and unmanned autonomous vehicle models. The simulation results show that the approach applied allows detecting vehicles with bogus data and excluding them from the group, thus increasing the safety of the intersection traversal by other vehicles.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Addresses V2X attacks or misbehaviour/trust evaluation that our engine can execute and detect in closed loop; it does not model the SCMS, revocation or joint traffic/radio/PKI ground truth.
**Novelty implications:** Does not rule out our candidate directions; any claim about this specific attack/detection/trust mechanism must cite it. Left open: end-to-end detection-report-revocation evaluation with safety outcomes.
