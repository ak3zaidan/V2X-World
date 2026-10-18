# Analysis of Realistic Attack Scenarios in Vehicle Ad-hoc Networks
Ján Laštinec, Mario Keszeli · International Symposium on Digital Forensics and Security (ISDFS) 2019 · https://doi.org/10.1109/isdfs.2019.8757500 · relevance: medium · basis: abstract

**Problem:** The pace of technological development in automotive and transportation has been accelerating rapidly in recent years. Automation of driver assistance systems, autonomous driving, increasing vehicle connectivity and emerging inter-vehicular communication (V2V) are among the most disruptive innovations, the latter of which also raises numerous unprecedented security concerns.
**Approach:** This paper is focused on the security of V2V communication in vehicle ad-hoc networks (VANET) with the main goal of identifying realistic attack scenarios and evaluating their impact, as well as possible security countermeasures to thwart the attacks. The paper also presents a simple detection method of Sybil nodes based on measuring the signal strength of received messages and maintaining reputation of sending nodes.
**Tools and scale:** OMNeT++, Car-Hacking/CAN dataset (mentioned in abstract)
**Results:** The evaluation has been done in OMNeT++ simulation environment and the results indicate that common attacks, such as replay attack or message falsification, can be eliminated by utilizing digital signatures and message validation.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Addresses V2X attacks or misbehaviour/trust evaluation that our engine can execute and detect in closed loop; it does not model the SCMS, revocation or joint traffic/radio/PKI ground truth.
**Novelty implications:** Does not rule out our candidate directions; any claim about this specific attack/detection/trust mechanism must cite it. Left open: end-to-end detection-report-revocation evaluation with safety outcomes.
