# Observer-Based Secure Control for Vehicular Platooning Under DoS Attacks
Sakineh Khodadadi, Tohid Kargar Tasooji, Horacio Jose Marquez · IEEE Access 2023 · https://doi.org/10.1109/access.2023.3250398 · relevance: medium · basis: abstract

**Problem:** This paper investigates an observer-based secure control problem for platooning of connected vehicles in the presence of Denial-of-Service (DoS) attacks. DoS attacks usually prevent the vehicle-to-vehicle data packets transmission which will lead to performance degradation of platooning system or vehicle collision.
**Approach:** To deal with DoS attacks, we consider an observer-based mechanism to estimate the state of vehicles based on available sensor measurements which significantly improves the resilience and tolerance of platooning system during the attack interval. Then, we provide the optimization framework to maximize the duration of the DoS attack such that the platooning system can tolerate safe operation without performance degradation.
**Tools and scale:** Car-Hacking/CAN dataset (mentioned in abstract)
**Results:** The simulation results verify the effectiveness of the proposed method.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Addresses V2X attacks or misbehaviour/trust evaluation that our engine can execute and detect in closed loop; it does not model the SCMS, revocation or joint traffic/radio/PKI ground truth.
**Novelty implications:** Does not rule out our candidate directions; any claim about this specific attack/detection/trust mechanism must cite it. Left open: end-to-end detection-report-revocation evaluation with safety outcomes.
