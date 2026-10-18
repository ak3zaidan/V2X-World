# Human Driver Temperament and the Safety Impact of a C-V2X Denial-of-Service Flooding Attack in Mixed-Autonomy Traffic
Rasheed Bello, Gurcan Comert, Varghese Vaidyan, Akinbobola Jegede, Vijay Bendigeri, Judith Mwakalonge · arXiv 2026 · https://arxiv.org/abs/2609.21206 · relevance: high · basis: full text (extracted sections)

**Problem:** The safety consequence of a DoS flood that blocks SPaT for CAVs depends on surrounding human drivers' behaviour.
**Approach:** Coupled OMNeT++/INET (5G NR-V2X) and SUMO simulation of a signalised corridor; sweep of CAV penetration (10-90%), four calibrated driver temperaments and two fail-safe policies (minimal-risk manoeuvre, ACC keep-driving), each attack arm differenced against a policy-matched no-attack baseline.
**Tools and scale:** OMNeT++/INET NR-V2X (channel is an analytical surrogate), SUMO; single stationary flooder.
**Results:** Baseline SPaT delivery falls from 0.61 (10%) to 0.26 (90%) from contention alone; flood drives it to about 0.18; aggressive surroundings raise hard braking under keep-driving fallback (p = 0.03, up to +8 episodes/1000 veh-s); temperament has no detectable effect on CAV red-light running or crossing conflicts.
**Limitations and future work (as stated by the authors):**
- Channel is an analytical surrogate whose absolute delivery ratios need field validation.
- Single stationary flooder attacker.
- Further caveats listed by the authors (four in total).
**Overlap with our engine:** strong - executes a V2X attack inside a coupled NR-V2X/traffic simulation and measures safety outcomes (hard braking, red-light running, conflicts) against matched no-attack baselines, i.e., attack-to-safety closed loop with paired comparisons; no PKI, MBD or revocation.
**Novelty implications:** We cannot claim to be first to quantify V2X attack safety impact in coupled NR-V2X/traffic simulation with matched baselines. Open: attacks that pass cryptographic checks (insider), the MBD-to-revocation loop, exact same-seed counterfactuals, and real PHY/MAC rather than an analytical channel surrogate.
